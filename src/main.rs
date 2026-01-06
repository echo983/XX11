#![recursion_limit = "256"]

mod orchestrator;
mod dsl;
mod llm;
mod state;
mod x11;

fn main() {
    if let Err(err) = orchestrator::run() {
        eprintln!("fatal: {err}");
        std::process::exit(1);
    }
}
