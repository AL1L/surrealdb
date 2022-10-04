use crate::err::Error;
use reqwest::blocking::Client;
use reqwest::blocking::Response;
use reqwest::header::ACCEPT;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use serde_json::Value;
use surrealdb::sql::parse;
use surrealdb::sql::statements::UseStatement;
use surrealdb::sql::Statement;

struct SqlContext {
	ns: Option<String>,
	db: Option<String>,
}

impl SqlContext {
	fn using(&mut self, statement: &UseStatement) {
		match statement.ns.clone() {
			Some(ns) => self.ns = Some(ns),
			None => {}
		};
		match statement.db.clone() {
			Some(db) => self.db = Some(db),
			None => {}
		};
	}
}

pub fn init(matches: &clap::ArgMatches) -> Result<(), Error> {
	// Set the default logging level
	crate::cli::log::init(3);
	// Parse all other cli arguments
	let user = matches.value_of("user").unwrap();
	let pass = matches.value_of("pass").unwrap();
	let conn = matches.value_of("conn").unwrap();
	let mut ctx = SqlContext {
		ns: matches.value_of("ns").map(|s| s.to_string()),
		db: matches.value_of("db").map(|s| s.to_string()),
	};

	// If we should pretty-print responses
	let pretty = matches.is_present("pretty");
	// Set the correct import URL
	let conn = format!("{}/sql", conn);
	// Create a new terminal REPL
	let mut rl = Editor::<()>::new().unwrap();
	// Load the command-line history
	let _ = rl.load_history("history.txt");
	// Loop over each command-line input
	loop {
		// Prompt the user to input SQL
		let readline = rl.readline("> ");
		// Check the user input
		match readline {
			// The user typed a query
			Ok(line) => {
				// Ignore all empty lines
				if line.is_empty() {
					continue;
				}
				// Add the entry to the history
				rl.add_history_entry(line.as_str());
				// Make a new remote request
				let res = Client::new()
					.post(&conn)
					.header(ACCEPT, "application/json")
					.basic_auth(user, Some(pass));
				// Add NS header if specified
				let res = match ctx.ns.clone() {
					Some(ns) => res.header("NS", ns),
					None => res,
				};
				// Add DB header if specified
				let res = match ctx.db.clone() {
					Some(db) => res.header("DB", db),
					None => res,
				};
				// Compile the request for later
				let ast = parse(line.as_str());
				// Complete request
				let res = res.body(line).send();
				// Get the request response
				match process(pretty, res) {
					Ok(v) => {
						println!("{}", v);
						if let Ok(ok_ast) = ast {
							for stmt in ok_ast.iter() {
								// If there's a use statement, update the context
								match stmt {
									Statement::Use(using) => ctx.using(using),
									_ => {}
								}
							}
						};
					}
					Err(e) => eprintln!("{}", e),
				}
			}
			// The user types CTRL-C
			Err(ReadlineError::Interrupted) => {
				break;
			}
			// The user typed CTRL-D
			Err(ReadlineError::Eof) => {
				break;
			}
			// There was en error
			Err(err) => {
				eprintln!("Error: {:?}", err);
				break;
			}
		}
	}
	// Save the inputs to the history
	let _ = rl.save_history("history.txt");
	// Everything OK
	Ok(())
}

fn process(pretty: bool, res: reqwest::Result<Response>) -> Result<String, Error> {
	// Catch any errors
	let res = res?;
	// Process the TEXT response
	let res = res.text()?;
	// Check if we should prettify
	match pretty {
		// Don't prettify the response
		false => Ok(res),
		// Yes prettify the response
		true => match res.is_empty() {
			// This was an empty response
			true => Ok(res),
			// Let's make this response pretty
			false => {
				// Parse the JSON response
				let res: Value = serde_json::from_str(&res)?;
				// Pretty the JSON response
				let res = serde_json::to_string_pretty(&res)?;
				// Everything processed OK
				Ok(res)
			}
		},
	}
}
