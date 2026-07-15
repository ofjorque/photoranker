use clap::Parser;

/// PhotoRanker — curación fotográfica cuantitativa, inteligente y sin "cajas negras".
///
/// Los subcomandos (`init`, `burst-detect`, `cluster`, `tournament-next`, `export-xmp`, ...)
/// se agregan a partir de docs/fase1-ingesta.md en adelante — ver docs/cli-reference.md.
#[derive(Parser)]
#[command(name = "photoranker", version, about)]
struct Cli;

fn main() {
    Cli::parse();
}
