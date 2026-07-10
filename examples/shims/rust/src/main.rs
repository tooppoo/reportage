use std::process::ExitCode;

// ExitCode is returned instead of calling std::process::exit so main unwinds normally.
// Both flush LLVM coverage data, but a normal return also keeps the example free of an abrupt-exit pattern that would skip destructors in a real application.
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    println!("sample: args {args:?}");

    if args.first().map(String::as_str) != Some("greet") {
        println!("Unknown command");
        return ExitCode::from(1);
    }
    greet(args.get(1).map(String::as_str).unwrap_or(""))
}

fn greet(msg: &str) -> ExitCode {
    println!("sample: greet {msg}");
    match msg {
        "hello" => {
            println!("Hello, World!");
            ExitCode::SUCCESS
        }
        "goodbye" => {
            println!("Goodbye, World!");
            ExitCode::SUCCESS
        }
        _ => {
            println!("sample: Unknown command");
            ExitCode::from(2)
        }
    }
}
