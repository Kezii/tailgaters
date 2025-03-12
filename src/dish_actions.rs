use std::sync::Arc;

use crate::{dish_controller::DishState, dish_driver::DishCommand, MainCommand};

pub struct Sweep1DParams {
    pub start: i32,
    pub end: i32,
    pub step: i32,
}

pub struct DishPosition {
    pub azimuth_count: i32,
    pub elevation_count: i32,
}

pub enum DishAction {
    ElevationSweep(Sweep1DParams),
    MoveAngles(f64, f64),
}

pub struct ActionManager {
    rx_channel: crossbeam::channel::Receiver<MainCommand>,
    tx_channel: crossbeam::channel::Sender<MainCommand>,
    state: std::sync::Arc<std::sync::Mutex<DishState>>,
}

impl ActionManager {
    pub fn new(
        rx_channel: crossbeam::channel::Receiver<MainCommand>,
        tx_channel: crossbeam::channel::Sender<MainCommand>,
        state: Arc<std::sync::Mutex<DishState>>,
    ) -> ActionManager {
        ActionManager {
            rx_channel,
            tx_channel,
            state,
        }
    }

    pub fn render(&self, action: DishAction) {
        match action {
            DishAction::ElevationSweep(params) => {
                let mut current = params.start;
                while current <= params.end {
                    self.tx_channel
                        .send(MainCommand::DishCommand(DishCommand::SetElevationAngle(
                            current as f64,
                        )))
                        .unwrap();
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    self.tx_channel
                        .send(MainCommand::DishCommand(DishCommand::RfWatch(1)))
                        .unwrap();
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                    current += params.step;
                }
            }
            DishAction::MoveAngles(az, el) => {
                self.set_position_blocking(az, el);
            }
        }
    }

    pub fn set_azimuth_blocking(&self, angle: f64) {
        self.tx_channel
            .send(MainCommand::DishCommand(DishCommand::SetAzimuthAngle(
                angle,
            )))
            .unwrap();

        while (self.state.lock().unwrap().azimuth_angle - angle).abs() > 0.1 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    pub fn set_elevation_blocking(&self, angle: f64) {
        self.tx_channel
            .send(MainCommand::DishCommand(DishCommand::SetElevationAngle(
                angle,
            )))
            .unwrap();

        while (self.state.lock().unwrap().elevation_angle - angle).abs() > 0.1 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    pub fn set_position_blocking(&self, az: f64, el: f64) {
        self.tx_channel
            .send(MainCommand::DishCommand(DishCommand::SetAzimuthAngle(az)))
            .unwrap();
        self.tx_channel
            .send(MainCommand::DishCommand(DishCommand::SetElevationAngle(el)))
            .unwrap();

        while (self.state.lock().unwrap().azimuth_angle - az).abs() > 0.1
            || (self.state.lock().unwrap().elevation_angle - el).abs() > 0.1
        {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    pub fn get_rf_power_blocking(&self) {
        self.tx_channel
            .send(MainCommand::DishCommand(DishCommand::RfWatch(1)))
            .unwrap();
    }
}
