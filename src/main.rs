use std::{
	env,
	fmt::{self, Display, Formatter},
	fs::File,
	io::{self, BufReader, Read},
};

use vte::{Params, Parser, Perform};

const BUFF_SIZE: usize = 1024;

//ANSI Escape Sequences
//https://gist.github.com/fnky/458719343aabd01cfb17a3a4f7296797

#[derive(Debug)]
enum VTTileData {
	GlyphStart(u16, u16), // (tile, flags)
	// Glyph flags
	// 0x0001: corpse
	// 0x0002: invisible
	// 0x0004: detected
	// 0x0008: pet
	// 0x0010: ridden
	// 0x0020: statue
	// 0x0040: object pile
	// 0x0080: lava hilight

	// The following information pertains to an upcoming version (Nethack
	// 3.7.0). If this version is now released, please verify that it is still
	// accurate, then update the page to incorporate this information.
	// https://nethackwiki.com/wiki/Vt_tiledata
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
///
/// Accumulate VT100 events (bytes) and emit events related to what happens.
///
/// This is done by maintaining an internal representation of the map state to
/// detect when something moves on the map.
struct Logger {
	selected_window: u16,
	current_message: Vec<char>,

	current_glyph: char,

	cursor_x: u16,
	cursor_y: u16,

	map_buffer: [char; 1600],
}

impl Display for Logger {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
		for n in (0..self.map_buffer.len()).step_by(80) {
			writeln!(
				f,
				"{}",
				self.map_buffer[n..n + 80].iter().collect::<String>()
			)?;
		}

		Ok(())
	}
}

impl Default for Logger {
	fn default() -> Self {
		let map_buffer = [' '; 1600];

		Self {
			selected_window: 255,
			current_message: Vec::new(),

			current_glyph: ' ',

			cursor_x: 0,
			cursor_y: 0,

			map_buffer,
		}
	}
}

impl Perform for Logger {
	fn print(&mut self, c: char) {
		self.current_glyph = c;
		self.current_message.push(c);

		if self.selected_window == 3 {
			println!("{},{}: {}", self.cursor_y, self.cursor_x, c);

			self.map_buffer[(self.cursor_y * 80 + self.cursor_x) as usize] = c;
		}

		self.cursor_x += 1;
	}

	fn execute(&mut self, _byte: u8) {
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
				//println!("cursormovement up: {:?}", params);
				self.cursor_y -= params.iter().next().expect("expected movement parameter")[0];
			}
			'B' => {
				//println!("cursormovement down");
				self.cursor_y += params.iter().next().expect("expected movement parameter")[0];
			}
			'C' => {
				//println!("cursormovement right");
				self.cursor_x += params.iter().next().expect("expected movement parameter")[0];
			}
			'D' => {
				//println!("cursormovement left");
				self.cursor_x -= params.iter().next().expect("expected movement parameter")[0];
			}
			'H' => {
				//println!("Move cursor to: {:?}", params);
				let mut params_iterator = params.iter().map(|s| s[0]);
				self.cursor_y = params_iterator.next().unwrap_or(0);
				self.cursor_x = params_iterator.next().unwrap_or(0);
			}
			/*
			'J' => {
				//println!("Erase-related");
			}
			'K' => {
				//println!("Erase in line");
			}
			'm' => {
				//println!("Set color/graphics");
			}
			*/
			'z' => {
				let data: VTTileData = params.into();
				println!("tiledata: {:?}", data);

				match data {
					VTTileData::WindowSelect(n) => {
						if self.selected_window == 3 {
							println!("{}", self);
						}
						if self.current_message.len() > 0 {
							let message: String = self.current_message.iter().cloned().collect();
							println!(
								"-{}---------- \n{}\n-------------",
								self.selected_window, message
							);
							self.current_message.clear();
						}
						self.selected_window = n;
					}
					VTTileData::GlyphStart(_, _) => {
						if self.selected_window != 3 {
							panic!("glyph started outside of map window");
						}
					}
					VTTileData::GlyphEnd => {
						println!("glyph: {}", self.current_glyph);
					}
					VTTileData::DataEnd => {
						println!("{}", self);
					}
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

	println!("{}", performer);

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	mod cursor_movement_a_up {
		use super::*;

		#[test]
		fn test_single_a_1_should_decrement_y() {
			// Given
			let input: &str = "\x1B[1A";

			let mut start_state = Logger::default();
			start_state.cursor_y = 42;

			let mut statemachine = Parser::new();

			// When
			for byte in input.bytes() {
				statemachine.advance(&mut start_state, byte);
			}

			// Then
			assert_eq!(41, start_state.cursor_y);
		}
	}

	mod cursor_movement_b_down {
		use super::*;

		#[test]
		fn test_single_b_1_should_increment_y() {
			// Given
			let input: &str = "\x1B[1B";

			let mut start_state = Logger::default();
			start_state.cursor_y = 42;

			let mut statemachine = Parser::new();

			// When
			for byte in input.bytes() {
				statemachine.advance(&mut start_state, byte);
			}

			// Then
			assert_eq!(43, start_state.cursor_y);
		}
	}

	mod cursor_movement_c_right {
		use super::*;

		#[test]
		fn test_single_c_4_should_increment_x_with_4() {
			// Given
			let input: &str = "\x1B[4C";

			let mut start_state = Logger::default();
			start_state.cursor_x = 12;

			let mut statemachine = Parser::new();

			// When
			for byte in input.bytes() {
				statemachine.advance(&mut start_state, byte);
			}

			// Then
			assert_eq!(16, start_state.cursor_x);
		}
	}

	mod cursor_movement_d_left {
		use super::*;

		#[test]
		fn test_single_d_7_should_increment_x_with_7() {
			// Given
			let input: &str = "\x1B[7D";

			let mut start_state = Logger::default();
			start_state.cursor_x = 12;

			let mut statemachine = Parser::new();

			// When
			for byte in input.bytes() {
				statemachine.advance(&mut start_state, byte);
			}

			// Then
			assert_eq!(5, start_state.cursor_x);
		}
	}

	mod cursor_movement_h_location {
		use super::*;

		#[test]
		fn test_single_h_should_increment_move_to_0_0() {
			// Given
			let input: &str = "\x1B[H";

			let mut start_state = Logger::default();
			start_state.cursor_x = 12;
			start_state.cursor_y = 28;

			let mut statemachine = Parser::new();

			// When
			for byte in input.bytes() {
				statemachine.advance(&mut start_state, byte);
			}

			// Then
			assert_eq!(0, start_state.cursor_x);
			assert_eq!(0, start_state.cursor_y);
		}

		#[test]
		fn test_single_line_column_h_should_increment_move_to_position() {
			// Given
			let input: &str = "\x1B[23;45H";

			let mut start_state = Logger::default();
			start_state.cursor_x = 12;
			start_state.cursor_y = 28;

			let mut statemachine = Parser::new();

			// When
			for byte in input.bytes() {
				statemachine.advance(&mut start_state, byte);
			}

			// Then
			assert_eq!(45, start_state.cursor_x);
			assert_eq!(23, start_state.cursor_y);
		}
	}
}
