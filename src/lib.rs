pub mod renderer;
pub mod ffmpeg;

use renderer::video_to_ascii;

pub fn run() {
    let args: Vec<_> = std::env::args().collect();

    if args.len() != 2 {
        print_usage();
        return;
    }

    let file_path = &args[1];

    if let Err(err) = video_to_ascii(&file_path) {
        println!("{}", err);
    };
}

fn print_usage() {
    println!("Usage: render [file-path]");
}