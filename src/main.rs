use std::env;
use std::fs::File;
use std::io::{self, BufReader, Read};

use vte::{Params, Parser, Perform};

const BUFF_SIZE: usize = 1024;

//ANSI Escape Sequences
//https://gist.github.com/fnky/458719343aabd01cfb17a3a4f7296797

#[derive(Debug)]
enum VTTileData {
	GlyphStart(u16, u32),  // (tile, flags)
    // Glyph flags
    // 0x0001: corpse
    // 0x0002: invisible
    // 0x0004: detected
    // 0x0008: pet
    // 0x0010: ridden
    // 0x0020: statue
    // 0x0040: object pile
    // 0x0080: lava hilight

    // The following information pertains to an upcoming version (Nethack 3.7.0). If this version is now released, please verify that it is still accurate, then update the page to incorporate this information.
    // 0x0100: ice hilight
    // 0x0200: out-of-sight lit areas when dark_room is disabled
    // 0x0400: unexplored
    // 0x0800: female
    // 0x1000: bad coordinates

	GlyphEnd,

	WindowSelect(u16),
    // 0: Prompts
    // 1: Messages
    // 2: Character stats
    // 3: Map?
    // 5: Dialogs?
	// 6: Seems to be "fullscreen" messages

	DataEnd,
}

impl From<&Params> for VTTileData {
	fn from(params: &Params) -> Self {
		let mut iterator = params.iter();
		let data = match iterator.next() {
			Some([1]) => match iterator.next() {
				Some([0]) => {
					if let (Some([n]), Some([m])) = (iterator.next(), iterator.next()) {
						Some(VTTileData::GlyphStart(*n, *m))
					} else {
						None
					}
				}
				Some([1]) => Some(VTTileData::GlyphEnd),
				Some([2]) => Some(VTTileData::WindowSelect(iterator.next().unwrap()[0])),
				Some([3]) => Some(VTTileData::DataEnd),
				p => {
					println!("Unknown parameter: {:?}", p);
					None
				}
			},
			p => {
				println!("Unknown parameter: {:?}", p);
				None
			}
		};
		if let Some(tile_data) = data {
			tile_data
		} else {
			panic!("unknown tile data {:?}", data)
		}
	}
}

/// Test implementation to develop ansi escape parsing
#[derive(Default)]
struct Logger {
	selected_window: u16,
	current_message: Vec<char>,
}

impl Perform for Logger {
	fn print(&mut self, c: char) {
		self.current_message.push(c);
	}

	fn execute(&mut self, byte: u8) {
		//println!("[execute] {:02x}", byte);
	}

	fn hook(&mut self, params: &Params, intermediates: &[u8], ignore: bool, c: char) {
		println!(
			"[hook] params={:?}, intermediates={:?}, ignore={:?}, char={:?}",
			params, intermediates, ignore, c
		);
	}

	fn put(&mut self, byte: u8) {
		println!("[put] {:02x}", byte);
	}

	fn unhook(&mut self) {
		println!("[unhook]");
	}

	fn osc_dispatch(&mut self, params: &[&[u8]], bell_terminated: bool) {
		println!(
			"[osc_dispatch] params={:?} bell_terminated={}",
			params, bell_terminated
		);
	}

	fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, c: char) {
		if ignore {
			return;
		}
		match c {
			'A' => {
				//println!("cursormovement up");
			}
			'B' => {
				//println!("cursormovement down");
			}
			'C' => {
				//println!("cursormovement right");
			}
			'D' => {
				//println!("cursormovement left");
			}
			'H' => {
				//println!("Move cursor to (0, 0");
			}
			'J' => {
				//println!("Erase-related");
			}
			'K' => {
				//println!("Erase in line");
			}
			'm' => {
				//println!("Set color/graphics");
			}
			'z' => {
				let data: VTTileData = params.into();
				println!("tiledata: {:?}", data);

				match data {
					VTTileData::WindowSelect(n) => {
                        if self.current_message.len() > 0 {
                            let message: String = self.current_message.iter().cloned().collect();
                            println!("-{}---------- \n{}\n-------------", self.selected_window, message);
                            self.current_message.clear();
                        }
                        self.selected_window = n;
                    },
					_ => (),
				};
			}
			_ => {
				println!(
					"[csi_dispatch] params={:#?}, intermediates={:?}, ignore={:?}, char={:?}",
					params, intermediates, ignore, c
				);
				println!("Unknown data: {}", c);
			}
		};
	}

	fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8) {
		println!(
			"[esc_dispatch] intermediates={:?}, ignore={:?}, byte={:02x}",
			intermediates, ignore, byte
		);
	}
}

fn main() -> io::Result<()> {
	let args: Vec<String> = env::args().collect();

	// Optionally use BufReader::with_capacity() if larger than 8kB buffer is needed
	let mut reader = BufReader::new(File::open(&args[1])?);
	let mut buffer = [0; BUFF_SIZE];

	let mut statemachine = Parser::new();
	let mut performer = Logger::default();

	while let Ok(n) = reader.read(&mut buffer[..]) {
		if 0 == n {
			break;
		}

		println!("Read {} bytes from file", n);

		for byte in &buffer[..n] {
			statemachine.advance(&mut performer, *byte);
		}
	}

	Ok(())
}
