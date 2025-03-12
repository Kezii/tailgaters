use clap::Parser;
use dish_controller::{DishSerialController, DishState};
use dish_driver::DishResponse;
use log::{info, trace, warn, LevelFilter};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph, Widget};
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::time::Duration;
use tui_logger::{init_logger, set_default_level, TuiLoggerSmartWidget};

mod dish_actions;
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
    #[arg(long, default_value = "1")]
    step: f64,
    #[arg(long)]
    scan: bool,
}

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{DefaultTerminal, Frame};

#[derive(Debug)]
pub struct RfPowerSample {
    pub power: f64,
    pub azimuth: f64,
    pub elevation: f64,
    pub time: std::time::Instant,
}

#[derive(Debug)]
pub enum GlobalBus {
    KeyboardEvent(KeyEvent),
    DishCommand(dish_driver::DishCommand),
    DishResponse(DishResponse),
    RfPowerSample(RfPowerSample),
    Update,
}

pub struct App {
    should_quit: bool,
    dish: DishSerialController,
    state: std::sync::Arc<std::sync::RwLock<DishState>>,
    channel_tx: crossbeam::channel::Sender<GlobalBus>,
    channel_rx: crossbeam::channel::Receiver<GlobalBus>,
    //actions_list: Vec<dish_actions::DishAction>,
    _actions_sender: crossbeam::channel::Sender<dish_actions::DishAction>,
    actions_receiver: crossbeam::channel::Receiver<dish_actions::DishAction>,
}

fn parse_cli_args() -> Result<(Cli, Vec<dish_actions::DishAction>)> {
    let args = Cli::parse();

    let mut actions_array = vec![];

    if args.scan {
        actions_array.push(dish_actions::DishAction::Scan2d(
            dish_actions::Scan2DParams {
                bottom_left: dish_actions::DishPosition {
                    azimuth: args.az_start as f64,
                    elevation: args.el_start as f64,
                },
                top_right: dish_actions::DishPosition {
                    azimuth: args.az_end as f64,
                    elevation: args.el_end as f64,
                },
                step: args.step,
            },
        ));
    }

    Ok((args, actions_array))
}

impl App {
    fn new(args: Cli, actions: Vec<dish_actions::DishAction>) -> Result<Self> {
        init_logger(LevelFilter::Debug)?;
        set_default_level(LevelFilter::Debug);
        info!("Starting up...");

        let (tx, rx) = crossbeam::channel::unbounded();

        let mut dish = DishSerialController::new(&args.port, args.baudrate, tx.clone()).unwrap();

        dish.send_command(dish_driver::DishCommand::Version)
            .unwrap();

        std::thread::sleep(Duration::from_millis(1000));

        let state = DishState {
            azimuth_count: 0,
            azimuth_angle: 0.0,
            elevation_count: 0,
            elevation_angle: 0.0,
            signal_strength: 0.0,
        };

        let state = std::sync::Arc::new(std::sync::RwLock::new(state));

        let (actions_sender, actions_receiver) = crossbeam::channel::unbounded();

        for action in actions {
            actions_sender.send(action).unwrap();
        }

        Ok(Self {
            should_quit: false,
            dish,
            state,
            channel_tx: tx,
            channel_rx: rx,
            //actions_list,
            _actions_sender: actions_sender,
            actions_receiver,
        })
    }

    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        self.start_keyboard_thread()?;
        self.start_actions_thread()?;

        let start_time_string = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        while !self.should_quit {
            let recv = self.channel_rx.recv();
            trace!("Received: {:?}", recv);
            match recv {
                Ok(GlobalBus::KeyboardEvent(key_event)) => {
                    self.handle_key_event(key_event);
                }
                Ok(GlobalBus::Update) => {}

                Ok(GlobalBus::DishResponse(response)) => {
                    self.state.write().unwrap().update_from_response(&response);

                    if let DishResponse::RfPower(pow) = response {
                        let rf_power_sample = RfPowerSample {
                            power: pow,
                            azimuth: self.state.read().unwrap().azimuth_angle,
                            elevation: self.state.read().unwrap().elevation_angle,
                            time: std::time::Instant::now(),
                        };
                        self.channel_tx
                            .send(GlobalBus::RfPowerSample(rf_power_sample))
                            .unwrap();
                    }
                }

                Ok(GlobalBus::DishCommand(command)) => {
                    self.dish.send_command(command).unwrap();
                }

                Ok(GlobalBus::RfPowerSample(power)) => {
                    info!(
                        "Power: {}, Azimuth: {:.4}, Elevation: {:.4}",
                        power.power, power.azimuth, power.elevation
                    );

                    if power.power > 5000.0 {
                        warn!("what the hell? power is too high");

                        self.dish
                            .send_command(dish_driver::DishCommand::RfWatch(1))
                            .unwrap();

                        std::thread::sleep(Duration::from_secs(1));

                        continue;
                    }

                    let mut file = OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(format!("rf_power_{}.csv", start_time_string))
                        .unwrap();

                    writeln!(
                        file,
                        "{},{},{},{}",
                        power.time.elapsed().as_secs(),
                        power.power,
                        power.azimuth,
                        power.elevation
                    )
                    .unwrap();
                }

                Err(_) => {}
            }
            terminal.draw(|frame| self.draw(frame))?;
        }

        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn start_keyboard_thread(&mut self) -> io::Result<()> {
        let sender_clone = self.channel_tx.clone();
        std::thread::spawn(move || {
            loop {
                match event::read().unwrap() {
                    // it's important to check that the event is a key press event as
                    // crossterm also emits key release and repeat events on Windows.
                    Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                        sender_clone
                            .send(GlobalBus::KeyboardEvent(key_event))
                            .unwrap();
                    }
                    _ => {}
                };
            }
        });

        Ok(())
    }

    fn start_actions_thread(&mut self) -> io::Result<()> {
        let recv_clone = self.actions_receiver.clone();

        let actions = dish_actions::ActionManager::new(self.channel_tx.clone(), self.state.clone());

        std::thread::spawn(move || loop {
            if let Ok(action) = recv_clone.recv() {
                info!("Executing action: {:#?}", action);
                actions.render(action);
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
            .constraints(vec![Constraint::Fill(1), Constraint::Length(3)])
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
            let state = self.state.read().unwrap();

            let state_text = vec![
                Line::from("Port: "),
                Line::from(self.dish.serial_port_name.clone().yellow()),
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
            .render(main_layout[1], buf);
    }
}

fn main() -> io::Result<()> {
    let (args, actions) = parse_cli_args().unwrap();

    color_eyre::install().unwrap();
    let mut terminal = ratatui::init();
    let mut app = App::new(args, actions).unwrap();
    let app_result = app.run(&mut terminal);
    ratatui::restore();
    app_result
}
