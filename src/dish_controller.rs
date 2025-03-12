use log::{error, info, trace};
use regex::Regex;
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::{
    error::Error,
    io::{BufRead, BufReader},
    thread,
    time::Duration,
};

use crate::{
    dish_driver::{DishCommand, DishResponse},
    GlobalBus,
};

#[derive(Debug)]
pub struct DishState {
    pub azimuth_count: i32,
    pub azimuth_angle: f64,
    pub elevation_count: i32,
    pub elevation_angle: f64,
    pub signal_strength: i32,
}

impl DishState {
    pub fn update_from_response(&mut self, response: &DishResponse) {
        match response {
            DishResponse::Azimuth(az, az_angle) => {
                self.azimuth_count = *az;
                self.azimuth_angle = *az_angle;
            }
            DishResponse::Elevation(el) => {
                self.elevation_count = *el;
                self.elevation_angle = Self::elevation_count_to_angle(*el);
            }
            DishResponse::RfPower(rf) => {
                self.signal_strength = *rf;
            }
            DishResponse::Ver(_) => {}
        }
    }

    pub fn elevation_angle_to_count(angle: f64) -> i32 {
        let count = 334.0 + angle * (1487.0 - 334.0) / 70.0;
        count as i32
    }

    pub fn elevation_count_to_angle(count: i32) -> f64 {
        70.0 * (count as f64 - 334.0) / (1487.0 - 334.0)
    }
}

/// DishController: an abstraction for controlling the dish over serial.
pub struct DishSerialController {
    serial_port: Box<dyn SerialPort>,
    pub serial_port_name: String,
    pub baudrate: u32,
    pub mainchan_sender: crossbeam::channel::Sender<GlobalBus>,
}

impl DishSerialController {
    /// Create and connect the DishController, opening the specified serial port.
    pub fn new(
        port_name: &str,
        baudrate: u32,
        channel: crossbeam::channel::Sender<GlobalBus>,
    ) -> Result<DishSerialController, Box<dyn Error>> {
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

        let mut res = DishSerialController {
            serial_port: sp,
            mainchan_sender: channel,
            serial_port_name: port_name.to_string(),
            baudrate,
        };

        res.rx_thread();
        res.tx_thread();

        Ok(res)
    }

    fn tx_thread(&self) {
        let sender_clone = self.mainchan_sender.clone();

        thread::spawn(move || {
            loop {
                // this thread just constantly asks for the azimuth and elevation
                // response is handled by the rx_thread

                if let Err(e) = sender_clone.send(GlobalBus::DishCommand(DishCommand::GetAzimuth))
                {
                    error!("{:?}", e);
                }
                if let Err(e) =
                    sender_clone.send(GlobalBus::DishCommand(DishCommand::GetElevation))
                {
                    error!("{:?}", e);
                }

                thread::sleep(Duration::from_millis(100));
            }
        });
    }

    fn rx_thread(&mut self) {
        let rx_port = self.serial_port.try_clone().unwrap();

        let sender = self.mainchan_sender.clone();
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

                    let dish_response = DishResponse::parse(&input_line);
                    if let Some(dr) = dish_response {
                        sender.send(GlobalBus::DishResponse(dr)).unwrap();
                    }

                    input_line.clear();
                }
            }
        });
    }

    pub fn send_command(&mut self, command: DishCommand) -> Result<(), Box<dyn Error>> {
        let cmd_str = command.serialize();
        for ch in cmd_str.chars() {
            self.serial_port.write_all(ch.to_string().as_bytes())?;
        }
        self.serial_port.write_all(b"\r")?;
        self.serial_port.flush()?;

        Ok(())
    }
}
