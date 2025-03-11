use log::{error, info, trace};
use regex::Regex;
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::{
    error::Error,
    io::{BufRead, BufReader},
    thread,
    time::Duration,
};

use crate::dish_driver::{DishCommand, DishResponse};

#[derive(Debug)]
pub struct DishState {
    pub serial_port: String,
    pub azimuth_count: i32,
    pub azimuth_angle: f64,
    pub elevation_count: i32,
    pub elevation_angle: f64,
    pub signal_strength: i32,
}

/// DishController: an abstraction for controlling the dish over serial.
pub struct DishController {
    serial_port: Box<dyn SerialPort>,
    pub state: std::sync::Arc<std::sync::Mutex<DishState>>,
}

impl DishController {
    /// Create and connect the DishController, opening the specified serial port.
    pub fn new(port_name: &str, baudrate: u32) -> Result<DishController, Box<dyn Error>> {
        // Configure the serial port options
        let sp = serialport::new(port_name, baudrate)
            .data_bits(DataBits::Eight)
            .flow_control(FlowControl::None)
            .stop_bits(StopBits::One)
            .parity(Parity::None)
            .timeout(Duration::from_secs(1))
            .open()?;

        info!(
            "Serial port '{}' opened at baudrate {}.",
            port_name, baudrate
        );

        let mut res = DishController {
            serial_port: sp,
            state: std::sync::Arc::new(std::sync::Mutex::new(DishState {
                serial_port: port_name.to_string(),
                azimuth_count: 0,
                azimuth_angle: 0.0,
                elevation_count: 0,
                elevation_angle: 0.0,
                signal_strength: 0,
            })),
        };

        res.rx_thread();
        res.tx_thread();

        Ok(res)
    }

    pub fn try_clone(&self) -> Result<DishController, Box<dyn Error>> {
        let sp = self.serial_port.try_clone()?;
        Ok(DishController {
            serial_port: sp,
            state: self.state.clone(),
        })
    }

    fn tx_thread(&self) {
        let mut selfclone = self.try_clone().unwrap();

        thread::spawn(move || {
            loop {
                // this thread just constantly asks for the azimuth and elevation
                // response is handled by the rx_thread
                if let Err(e) = selfclone.send_command(DishCommand::GetAzimuth) {
                    error!("{:?}", e);
                }
                if let Err(e) = selfclone.send_command(DishCommand::GetElevation) {
                    error!("{:?}", e);
                }

                thread::sleep(Duration::from_millis(100));
            }
        });
    }

    // Received: "Current heading:       3224 (160.192 deg.)\r\n"
    // Received: "Current elevation: 1098\r\n"
    // Received: "Current rfss:           \u{1b}[5D3142 \u{1b}[5D3142 \u{1b}[5D3141 \u{1b}[5D3141 \u{1b}[5D3142 \u{1b}[5D3140 \u{1b}[5D3140 \u{1b}[5D3140 \u{1b}[5D3140 \u{1b}[5D3141 \u{1b}[5D3140 \u{1b}[5D3140 \u{1b}[5D3141 \u{1b}[5D3140 \u{1b}[5D3141 \u{1b}[5D3141 \u{1b}[5D3140 \u{1b}[5D3142 \u{1b}[5D3140 \u{1b}[5D3141"

    fn rx_thread(&mut self) {
        let state = self.state.clone();
        let rx_port = self.serial_port.try_clone().unwrap();
        thread::spawn(move || {
            let mut reader = BufReader::with_capacity(1, rx_port);
            let mut input_line = String::new();
            loop {
                if reader.read_line(&mut input_line).is_ok() {
                    input_line = input_line.trim().to_string();

                    if input_line.is_empty() {
                        input_line.clear();
                        continue;
                    }

                    let mut state = state.lock().unwrap();

                    let dish_response = DishResponse::parse(&input_line);
                    if let Some(dr) = dish_response {
                        match dr {
                            DishResponse::Azimuth(az, az_angle) => {
                                state.azimuth_count = az;
                                state.azimuth_angle = az_angle;
                            }
                            DishResponse::Elevation(el) => {
                                state.elevation_count = el;
                                state.elevation_angle =
                                    DishController::elevation_count_to_angle(el);
                            }
                            DishResponse::Ver(ver) => {
                                trace!("Version: {}", ver);
                            }
                            DishResponse::RfPower(rf) => {
                                state.signal_strength = rf;
                            }
                        }
                    }

                    input_line.clear();
                }
            }
        });
    }

    /// Sends a command string one character at a time, then a carriage return.
    fn send_command_(&mut self, cmd_str: &str) -> Result<(), Box<dyn Error>> {
        for ch in cmd_str.chars() {
            self.serial_port.write_all(ch.to_string().as_bytes())?;
        }
        self.serial_port.write_all(b"\r")?;
        self.serial_port.flush()?;
        Ok(())
    }

    pub fn send_command(&mut self, command: DishCommand) -> Result<(), Box<dyn Error>> {
        let cmd_str = command.serialize();
        self.send_command_(&cmd_str)?;
        Ok(())
    }

    pub fn elevation_angle_to_count(angle: f64) -> i32 {
        let count = 334.0 + angle * (1487.0 - 334.0) / 70.0;
        count as i32
    }

    pub fn elevation_count_to_angle(count: i32) -> f64 {
        70.0 * (count as f64 - 334.0) / (1487.0 - 334.0)
    }
}
