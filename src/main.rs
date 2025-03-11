use clap::{Arg, Args, Command, Parser};
use dish_controller::DishController;
use log::{info, LevelFilter};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::symbols::border;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use regex::Regex;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;
use tui_input::Input;
use tui_logger::{init_logger, set_default_level, TuiLoggerSmartWidget};

// For image creation
use image::{ImageBuffer, Rgb, RgbImage};

mod dish_controller;
mod dish_driver;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long, default_value = "/dev/ttyACM0")]
    port: String,
    #[arg(short, long, default_value = "9600")]
    baudrate: u32,
    #[arg(long, default_value = "90")]
    az_start: i32,
    #[arg(long, default_value = "270")]
    az_end: i32,
    #[arg(long, default_value = "5")]
    el_start: i32,
    #[arg(long, default_value = "70")]
    el_end: i32,
    #[arg(long, default_value = "false")]
    scan: bool,
}

/// Writes a 2D array of i32 values to a text file, space-separated rows.
fn write_raw_data(file_name: &str, data: &Vec<Vec<i32>>) -> Result<(), Box<dyn Error>> {
    let mut file = File::create(file_name)?;
    for row in data.iter() {
        let row_str = row
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<String>>()
            .join(" ");
        writeln!(file, "{}", row_str)?;
    }
    Ok(())
}

fn main_() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    // Command-line argument parsing
    let args = Cli::parse();

    let mut az_start = args.az_start.clamp(0, 360);
    let mut az_end = args.az_end.clamp(0, 360);
    let mut el_start = args.el_start.clamp(5, 70);
    let mut el_end = args.el_end.clamp(5, 70);

    // Connect to dish
    let mut dish = DishController::new(&args.port, args.baudrate)?;

    // dish.az_angle(160)?;
    // thread::sleep(Duration::from_secs(1));
    // dish.version()?;
    // thread::sleep(Duration::from_secs(2));

    // dish.rfwatch(5)?;

    loop {
        println!("{:?}", dish.state.lock().unwrap());
        thread::sleep(Duration::from_millis(1000));
    }

    // // Perform scanning
    // if high_res == false {
    //     // Low resolution (azangle, elangle)
    //     let az_range = az_end - az_start;
    //     let el_range = el_end - el_start;

    //     // Provide runtime estimate
    //     let time_est = az_range.abs() * el_range.abs();
    //     let time_output = (time_est as f64 + (time_est as f64 / 6.0)) / 60.0;
    //     if time_output > 60.0 {
    //         println!("Estimated scan time: {:.2} hours", time_output / 60.0);
    //     } else {
    //         println!("Estimated scan time: {:.2} minutes", time_output);
    //     }
    //     println!("Starting low resolution scan...\n");

    //     // Create image (width = az_range+1, height = el_range+1)
    //     let width = (az_range.abs() + 1) as u32;
    //     let height = (el_range.abs() + 1) as u32;
    //     let mut img: RgbImage = ImageBuffer::new(width, height);

    //     // 2D array for raw data
    //     let mut sky_data = vec![vec![0i32; width as usize]; height as usize];

    //     // Move dish to starting position
    //     println!("Moving dish to starting position...");
    //     dish.az_angle(az_start)?;
    //     thread::sleep(Duration::from_secs(10));
    //     dish.move_el_angle(el_start)?;
    //     thread::sleep(Duration::from_secs(10));

    //     // Scanning loops
    //     for elevation in el_start..el_end {
    //         for azimuth in az_start..az_end {
    //             println!("Azimuth={}, Elevation={}", azimuth, elevation);
    //             dish.az_angle(azimuth)?;

    //             let strength = dish.read_signal_strength()?;
    //             println!("Signal: {}", strength);

    //             // Mirror the Python indexing:
    //             let row_idx = (elevation - el_end).abs() as usize;
    //             let col_idx = (azimuth - az_end).abs() as usize;

    //             sky_data[row_idx][col_idx] = strength;

    //             // Save data to text file each time
    //             let raw_file_name = format!("raw-data.txt");
    //             write_raw_data(&raw_file_name, &sky_data)?;

    //             // Update image pixel
    //             // Red channel is (strength % 255), green=0, blue=0
    //             let red_val = (strength % 255) as u8;
    //             img.put_pixel(col_idx as u32, row_idx as u32, Rgb([red_val, 0, 0]));

    //             // Save updated PNG
    //             let img_file_name = format!("result.png");
    //             img.save(&img_file_name)?;

    //             println!();
    //         }
    //         // Return to start azimuth
    //         dish.az_angle(az_start)?;
    //         // Delay based on range
    //         let wait_time = ((az_range.abs() as f64) * 0.05) as u64 + 1;
    //         thread::sleep(Duration::from_secs(wait_time));

    //         // Move dish up one degree
    //         // (Mirroring Python's "dish.move_el_angle(elevation)" after each row.)
    //         dish.move_el_angle(elevation)?;
    //     }
    // } else {
    //     // High resolution (nudge) ~0.2 deg az, 0.33 deg el
    //     let az_range = (az_end - az_start).abs() * 5;
    //     let el_range = (el_end - el_start).abs() * 3;

    //     let time_est = az_range * el_range;
    //     let time_output = ((time_est as f64 + (time_est as f64 / 6.0)) / 60.0)
    //         + ((el_range as f64 * 10.0) / 60.0);
    //     if time_output > 60.0 {
    //         println!("Estimated scan time: {:.2} hours", time_output / 60.0);
    //     } else {
    //         println!("Estimated scan time: {:.2} minutes", time_output);
    //     }
    //     println!("Starting high resolution scan...\n");

    //     // Create image
    //     let width = az_range + 1;
    //     let height = el_range + 1;
    //     let mut img: RgbImage = ImageBuffer::new(width as u32, height as u32);

    //     let mut sky_data = vec![vec![0i32; width as usize]; height as usize];

    //     println!("Moving dish to starting position...");
    //     dish.az_angle(az_start)?;
    //     thread::sleep(Duration::from_secs(10));
    //     dish.move_el_angle(el_start)?;
    //     thread::sleep(Duration::from_secs(10));

    //     // Scanning loops
    //     for elevation in 0..el_range {
    //         for azimuth in 0..az_range {
    //             println!("X={}, Y={}", azimuth, elevation);

    //             // Nudge az
    //             dish.nudge_az_ccw()?;

    //             let strength = dish.read_signal_strength()?;
    //             println!("Signal: {}", strength);

    //             let row_idx = (elevation as i32 - el_range as i32).abs() as usize;
    //             let col_idx = (azimuth as i32 - az_range as i32).abs() as usize;
    //             sky_data[row_idx][col_idx] = strength;

    //             let raw_file_name = format!("raw-data.txt");
    //             write_raw_data(&raw_file_name, &sky_data)?;

    //             let red_val = (strength % 255) as u8;
    //             img.put_pixel(col_idx as u32, row_idx as u32, Rgb([red_val, 0, 0]));

    //             let img_file_name = format!("result.png");
    //             img.save(&img_file_name)?;

    //             println!();
    //         }
    //         // Return to starting azimuth
    //         dish.az_angle(az_start)?;
    //         let wait_time = (((az_range as f64) / 5.0) * 0.05) as u64 + 1;
    //         thread::sleep(Duration::from_secs(wait_time));

    //         // Nudge elevation
    //         dish.nudge_el_up()?;
    //     }
    // }

    println!("Scan complete!");

    Ok(())
}

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{DefaultTerminal, Frame};

pub enum MainChannelType {
    KeyEvent(KeyEvent),
    Update,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    #[default]
    Normal,
    Editing,
}

pub struct App {
    should_quit: bool,
    dish: DishController,
    /// Current value of the input box
    input: Input,
    input_mode: InputMode,
    renderchan_sender: std::sync::mpsc::Sender<MainChannelType>,
    renderchan_receiver: std::sync::mpsc::Receiver<MainChannelType>,
}

impl App {
    pub fn new() -> Result<Self> {
        init_logger(LevelFilter::Debug)?;
        set_default_level(LevelFilter::Debug);
        info!("Starting up...");
        let args = Cli::parse();
        let mut az_start = args.az_start.clamp(0, 360);
        let mut az_end = args.az_end.clamp(0, 360);
        let mut el_start = args.el_start.clamp(5, 70);
        let mut el_end = args.el_end.clamp(5, 70);

        let mut dish = DishController::new(&args.port, args.baudrate).unwrap();

        dish.send_command(dish_driver::DishCommand::Version)
            .unwrap();

        std::thread::sleep(Duration::from_secs(2));

        let (sender, receiver) = channel();

        let chan_clone = sender.clone();
        std::thread::spawn(move || loop {
            chan_clone.send(MainChannelType::Update).unwrap();
            std::thread::sleep(Duration::from_millis(100));
        });

        Ok(Self {
            should_quit: false,
            dish,
            input: Input::default(),
            input_mode: InputMode::Normal,
            renderchan_sender: sender,
            renderchan_receiver: receiver,
        })
    }

    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        self.handle_events()?;

        while !self.should_quit {
            match self.renderchan_receiver.recv() {
                Ok(MainChannelType::KeyEvent(key_event)) => {
                    self.handle_key_event(key_event);
                }
                Ok(MainChannelType::Update) => {}
                Err(_) => {}
            }
            terminal.draw(|frame| self.draw(frame))?;
        }

        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        let sender_clone = self.renderchan_sender.clone();
        std::thread::spawn(move || {
            loop {
                match event::read().unwrap() {
                    // it's important to check that the event is a key press event as
                    // crossterm also emits key release and repeat events on Windows.
                    Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                        sender_clone
                            .send(MainChannelType::KeyEvent(key_event))
                            .unwrap();
                    }
                    _ => {}
                };
            }
        });

        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Left => self
                .dish
                .send_command(dish_driver::DishCommand::NudgeAzimuthCcw)
                .unwrap(),
            KeyCode::Right => self
                .dish
                .send_command(dish_driver::DishCommand::NudgeAzimuthCw)
                .unwrap(),
            KeyCode::Up => self
                .dish
                .send_command(dish_driver::DishCommand::NudgeElevationUp)
                .unwrap(),
            KeyCode::Down => self
                .dish
                .send_command(dish_driver::DishCommand::NudgeElevationDown)
                .unwrap(),
            KeyCode::Char(' ') => self
                .dish
                .send_command(dish_driver::DishCommand::RfWatch(1))
                .unwrap(),

            _ => {}
        }
    }

    fn exit(&mut self) {
        self.should_quit = true;
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Fill(1),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .split(area);

        let upper_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Fill(1), Constraint::Length(20)])
            .split(main_layout[0]);

        TuiLoggerSmartWidget::default()
            .style_error(Style::default().fg(Color::Red))
            .style_debug(Style::default().fg(Color::Green))
            .style_warn(Style::default().fg(Color::Yellow))
            .style_trace(Style::default().fg(Color::Magenta))
            .style_info(Style::default().fg(Color::Cyan))
            .output_separator(':')
            .output_timestamp(Some("%H:%M:%S".to_string()))
            .output_level(Some(tui_logger::TuiLoggerLevelOutput::Abbreviated))
            .output_target(true)
            .output_file(true)
            .output_line(true)
            //.state(self.selected_state())
            .render(upper_layout[0], buf);

        {
            let state = self.dish.state.lock().unwrap();
            let state_text = vec![
                Line::from("Port: "),
                Line::from(state.serial_port.clone().yellow()),
                Line::from("Azimuth (count): "),
                Line::from(state.azimuth_count.to_string().yellow()),
                Line::from("Elevation (count): "),
                Line::from(state.elevation_count.to_string().yellow()),
                Line::from("Azimuth: "),
                Line::from(format!("{:.4}°", state.azimuth_angle).yellow()),
                Line::from("Elevation: "),
                Line::from(format!("{:.4}°", state.elevation_angle).yellow()),
                Line::from("Signal: "),
                Line::from(state.signal_strength.to_string().yellow()),
            ];
            Paragraph::new(state_text)
                .block(Block::new())
                .render(upper_layout[1], buf);
        }

        let bottom_instructions = vec![
            Line::from(vec![
                " Nudge UP ".into(),
                "<Up>".blue().bold(),
                " Nudge DOWN ".into(),
                "<Down>".blue().bold(),
                " Nudge CCW ".into(),
                "<Left>".blue().bold(),
                " Nudge CW ".into(),
                "<Right>".blue().bold(),
            ]),
            Line::from(vec![" Read Signal Level ".into(), "<Space>".blue().bold()]),
            Line::from(vec![
                " Press ".into(),
                "<Q>".blue().bold(),
                " to exit the application.".into(),
            ]),
        ];

        Paragraph::new(bottom_instructions)
            .block(Block::new())
            .render(main_layout[2], buf);

        let input = Paragraph::new(self.input.value())
            //.style(style)
            .scroll((0, 0))
            .block(Block::bordered().title("Input"))
            .render(main_layout[1], buf);
    }
}

fn main() -> io::Result<()> {
    color_eyre::install().unwrap();
    let mut terminal = ratatui::init();
    let mut app = App::new().unwrap();
    let app_result = app.run(&mut terminal);
    ratatui::restore();
    app_result
}
