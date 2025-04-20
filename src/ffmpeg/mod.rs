use std::io::{BufRead, BufReader, Read};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;

pub struct FfmpegReader {
    pipe: Option<Child>,
    buf_reader: Option<BufReader<ChildStdout>>,
    frame_buffer: Vec<u8>,
    frame_size: usize,
    frame_header_size: usize,
    err_rx: Receiver<String>,
    err_thread_handle: Option<JoinHandle<()>>
}

impl FfmpegReader {
    pub fn new(video_file_path: &str, w: usize, h: usize) -> Result<Self, std::io::Error> {
        let mut cmd = Command::new("ffmpeg");

        cmd.arg("-i")
            .arg(video_file_path)
            .arg("-s")
            .arg(&format!("{}x{}", w, h))
            .arg("-f")
            .arg("image2pipe")
            .arg("-pix_fmt")
            .arg("rgb24")
            .arg("-vcodec")
            .arg("ppm")
            .arg("-nostats")
            .arg("-hide_banner")
            .arg("-");

        let mut ffmpeg = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

        let stdout = ffmpeg.stdout.take().unwrap();
        let stderr = ffmpeg.stderr.take().unwrap();

        let (error_tx, error_rx) = std::sync::mpsc::channel();

        let hnd = std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    // if line.contains("Error") {
                        let _ = error_tx.send(line);
                        break;
                    // }
                }
            }
        });
        
        // TODO: ugly
        let ppm_header_size = 9 + w.to_string().len() + h.to_string().len();
        let frame_size = ppm_header_size + w * h * 3;

        Ok(FfmpegReader {
            pipe: Some(ffmpeg),
            buf_reader: Some(BufReader::new(stdout)),
            frame_buffer: vec![],
            frame_size: frame_size,
            frame_header_size: ppm_header_size,
            err_rx: error_rx,
            err_thread_handle: Some(hnd)
        })
    }

    pub fn get_frame_buffer_ppm(&mut self) -> Option<&[u8]> {
        let b = &mut self.frame_buffer;

        if b.len() > 0 {
            b.clear();
        }

        let m = "Cannot get a frame buffer with uninitialized BufReader.";

        let bytes_read = self
            .buf_reader
            .as_mut()
            .expect(m)
            .by_ref()
            .take(self.frame_size as u64)
            .read_to_end(b)
            .unwrap(); 

        if bytes_read == 0 {
            return None;
        }

        Some(&b[self.frame_header_size..])
    }

    pub fn wait_for_error_thread(&mut self) -> Result<(), String> {
        let pipe = self
            .pipe
            .as_mut()
            .expect("Cannot wait for uninitialized pipe.");

        let exit_status = pipe.wait().map_err(|e| format!("Wait failed {}", e))?;

        let hnd = self.err_thread_handle.take().unwrap();
        
        hnd.join().map_err(|_| String::from("Couldn't join error thread."))?;

        if !exit_status.success() {
            let err = self.err_rx.try_recv().unwrap_or_default();
            return Err(format!("FFMPEG Error ({}): {}", exit_status, err));
        }
        
        Ok(())
    }
}