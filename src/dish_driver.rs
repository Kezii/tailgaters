use regex::Regex;
#[derive(Debug, PartialEq)]
pub enum DishCommand {
    SetAzimuthAngle(f64),
    GetAzimuth,
    GetElevation,
    SetElevationAngle(f64),
    NudgeAzimuthCcw,
    NudgeAzimuthCw,
    NudgeElevationUp,
    NudgeElevationDown,
    RfWatch(i32),
    Version,
}

#[derive(Debug, PartialEq)]
pub enum DishResponse {
    Azimuth(i32, f64),
    Elevation(i32),
    RfPower(f64),
    Ver(String),
}

/*
            ver        Prints console version number
           elev        Raise / lower dish to target elevation
         elevmt        Raise / lower dish to target elevation, and maintain
      elevwatch        Monitor the reported elevation
        elangle        Move to given elevation angle
        elnudge        Nudge the elevation up or down by approx 0.2 deg
          elacc        Raise / lower dish to target elevation accuarate
           azim        Rotate dish to target heading
      azimwatch        Monitor the reported heading
        azangle        Move to given azimuth angle
        aznudge        Nudge the azimuth motor clockwise or counterclockwise by approx 0.2 deg
          azacc        Move to given azimuth angle
        rfwatch        Monitor reported rf signal strength
            cal        Calibrate tilt sensor
           scan        Starts or stops a scan for RF at current elevation
            gps        Store terrestrial location
           stat        Statistic function testing
         nvread        Read NVRAM
        nvwrite        Write NVRAM
          reset        Reset the system

*/

impl DishCommand {
    pub fn serialize(&self) -> String {
        match self {
            DishCommand::SetAzimuthAngle(angle_degrees) => format!("azangle {}", angle_degrees),
            DishCommand::GetAzimuth => "azacc".to_string(),
            DishCommand::GetElevation => "elacc".to_string(),
            DishCommand::SetElevationAngle(angle) => format!("elangle {}", angle),
            DishCommand::NudgeAzimuthCcw => "aznudge ccw".to_string(),
            DishCommand::NudgeAzimuthCw => "aznudge cw".to_string(),
            DishCommand::NudgeElevationUp => "elnudge up".to_string(),
            DishCommand::NudgeElevationDown => "elnudge down".to_string(),
            DishCommand::RfWatch(time) => format!("rfwatch {}", time),
            DishCommand::Version => "ver".to_string(),
        }
    }
}

/*
// Received: "Current heading:       3224 (160.192 deg.)\r\n"
// Received: "Current elevation: 1098\r\n"
// Received: "Current rfss:           \u{1b}[5D3142 \u{1b}[5D3142 \u{1b}[5D3141 \u{1b}[5D3141 \u{1b}[5D3142 \u{1b}[5D3140 \u{1b}[5D3140 \u{1b}[5D3140 \u{1b}[5D3140 \u{1b}[5D3141 \u{1b}[5D3140 \u{1b}[5D3140 \u{1b}[5D3141 \u{1b}[5D3140 \u{1b}[5D3141 \u{1b}[5D3141 \u{1b}[5D3140 \u{1b}[5D3142 \u{1b}[5D3140 \u{1b}[5D3141"
// Received: "Stopped at Az: 3536"
*/

impl DishResponse {
    pub fn parse(line_from_dish: &str) -> Option<DishResponse> {
        let line = line_from_dish.trim();
        let parts: Vec<&str> = line.split_whitespace().collect();

        match line {
            s if s.starts_with("Current heading:") => {
                let az = parts[2].parse::<i32>().ok()?;
                let az_angle = parts[3]
                    .trim_start_matches('(')
                    .parse::<f64>()
                    .unwrap_or_default();
                Some(DishResponse::Azimuth(az, az_angle))
            }
            s if s.starts_with("Current elevation:") => {
                let el = parts[2].parse::<i32>().ok()?;
                Some(DishResponse::Elevation(el))
            }
            s if s.starts_with("Current rfss:") => {
                let parts: Vec<&str> = s.split("[5D").collect();

                let re_control = Regex::new(r"\p{C}").unwrap();
                let re_non_digits = Regex::new(r"[^\d]").unwrap();

                let parts_clean = parts.iter().skip(1).map(|p| {
                    // Remove control characters
                    let cleaned = re_control.replace_all(p, "");
                    // Remove everything but digits
                    let cleaned = re_non_digits.replace_all(&cleaned, "");
                    let cleaned = cleaned.trim();

                    cleaned.parse::<i32>().ok().unwrap()
                });

                let average = parts_clean.fold((0, 0), |(sum, count), val| (sum + val, count + 1));
                Some(DishResponse::RfPower(average.0 as f64 / average.1 as f64))
            }
            s if s.starts_with("GO>") => None,
            "azacc" | "elacc" => None,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dish_response_parse() {
        let line = "Current heading:       3224 (160.192 deg.)";
        let response = DishResponse::parse(line).unwrap();
        assert_eq!(response, DishResponse::Azimuth(3224, 160.192));

        let line = "Current elevation: 1098";
        let response = DishResponse::parse(line).unwrap();
        assert_eq!(response, DishResponse::Elevation(1098));

        let line = "Current rfss:           \u{1b}[5D3142 \u{1b}[5D3142 \u{1b}[5D3141 \u{1b}[5D3141 \u{1b}[5D3142";
        let response = DishResponse::parse(line).unwrap();
        assert_eq!(response, DishResponse::RfPower(3141.6));
    }
}
