use std::sync::Arc;

use log::{info, warn};

use crate::{dish_controller::DishState, dish_driver::DishCommand, GlobalBus};

#[derive(Debug)]
pub struct Sweep1DParams {
    pub start: i32,
    pub end: i32,
    pub step: i32,
}
#[derive(Debug)]
pub struct Scan2DParams {
    pub bottom_left: DishPosition,
    pub top_right: DishPosition,
    pub step: f64,
}
#[derive(Debug)]
pub struct DishPosition {
    pub azimuth: f64,
    pub elevation: f64,
}

#[derive(Debug)]
pub enum DishAction {
    ElevationSweep(Sweep1DParams),
    Scan2d(Scan2DParams),
    MoveAngles(f64, f64),
}

pub struct ActionManager {
    tx_channel: crossbeam::channel::Sender<GlobalBus>,
    state: std::sync::Arc<std::sync::RwLock<DishState>>,
}

impl ActionManager {
    pub fn new(
        tx_channel: crossbeam::channel::Sender<GlobalBus>,
        state: Arc<std::sync::RwLock<DishState>>,
    ) -> ActionManager {
        ActionManager { tx_channel, state }
    }

    pub fn render(&self, action: DishAction) {
        match action {
            DishAction::ElevationSweep(params) => {
                let mut current = params.start;
                while current <= params.end {
                    self.tx_channel
                        .send(GlobalBus::DishCommand(DishCommand::SetElevationAngle(
                            current as f64,
                        )))
                        .unwrap();
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    self.tx_channel
                        .send(GlobalBus::DishCommand(DishCommand::RfWatch(1)))
                        .unwrap();
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                    current += params.step;
                }
            }
            DishAction::MoveAngles(az, el) => {
                self.set_position_blocking(az, el);
            }
            DishAction::Scan2d(params) => {
                info!("Starting scan");

                for az in (params.bottom_left.azimuth as i32..params.top_right.azimuth as i32)
                    .step_by(params.step as usize)
                {
                    for el in (params.bottom_left.elevation as i32
                        ..params.top_right.elevation as i32)
                        .step_by(params.step as usize)
                    {
                        self.set_position_blocking(az as f64, el as f64);
                        self.tx_channel
                            .send(GlobalBus::DishCommand(DishCommand::RfWatch(1)))
                            .unwrap();
                        std::thread::sleep(std::time::Duration::from_millis(1000));
                    }
                }

                info!("Scan finished!!");

                self.set_position_blocking(
                    params.bottom_left.azimuth,
                    params.bottom_left.elevation,
                );

                info!("Exiting scan");
            }
        }
    }

    pub fn set_azimuth_blocking(&self, angle: f64) {
        self.tx_channel
            .send(GlobalBus::DishCommand(DishCommand::SetAzimuthAngle(angle)))
            .unwrap();

        while (self.state.read().unwrap().azimuth_angle - angle).abs() > 0.1 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    pub fn set_elevation_blocking(&self, angle: f64) {
        self.tx_channel
            .send(GlobalBus::DishCommand(DishCommand::SetElevationAngle(
                angle,
            )))
            .unwrap();

        while (self.state.read().unwrap().elevation_angle - angle).abs() > 0.1 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    pub fn set_position_blocking(&self, az: f64, el: f64) {
        self.tx_channel
            .send(GlobalBus::DishCommand(DishCommand::SetAzimuthAngle(az)))
            .unwrap();
        self.tx_channel
            .send(GlobalBus::DishCommand(DishCommand::SetElevationAngle(el)))
            .unwrap();

        let now = std::time::Instant::now();
        while (self.state.read().unwrap().azimuth_angle - az).abs() > 2.0
            || (self.state.read().unwrap().elevation_angle - el).abs() > 2.0
        {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if now.elapsed().as_secs() > 15 {
                warn!("Timeout while setting position, position will be imprecise");
                // try again
                self.tx_channel
                    .send(GlobalBus::DishCommand(DishCommand::SetAzimuthAngle(az)))
                    .unwrap();
                self.tx_channel
                    .send(GlobalBus::DishCommand(DishCommand::SetElevationAngle(el)))
                    .unwrap();

                break;
            }
        }
        info!("Set position to azimuth: {}, elevation: {}", az, el);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
