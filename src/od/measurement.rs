/*
    Nyx, blazing fast astrodynamics
    Copyright (C) 2023 Christopher Rabotin <christopher.rabotin@gmail.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU Affero General Public License as published
    by the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU Affero General Public License for more details.

    You should have received a copy of the GNU Affero General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/

use super::msr::StdMeasurement;
use super::TrackingDataSim;
use crate::cosmic::{Cosm, Frame, Orbit};
use crate::time::Epoch;
use crate::Spacecraft;
use rand_distr::Normal;
use std::fmt;
use std::sync::Arc;

#[cfg(feature = "python")]
use pyo3::prelude::*;
use rand::Rng;

/// GroundStation defines a Two Way ranging equipment.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "python", pyclass)]
pub struct GroundStation {
    pub name: String,
    /// in degrees
    pub elevation_mask_deg: f64,
    /// in degrees
    pub latitude_deg: f64,
    /// in degrees
    pub longitude_deg: f64,
    /// in km
    pub height_km: f64,
    /// Frame in which this station is defined
    pub frame: Frame,
    range_noise: Normal<f64>,
    range_rate_noise: Normal<f64>,
}

impl GroundStation {
    /// Initializes a new Two Way ranging equipment from the noise values.
    pub fn from_noise_values(
        name: String,
        elevation_mask: f64,
        latitude: f64,
        longitude: f64,
        height: f64,
        range_noise: f64,
        range_rate_noise: f64,
        frame: Frame,
    ) -> Self {
        Self {
            name,
            elevation_mask_deg: elevation_mask,
            latitude_deg: latitude,
            longitude_deg: longitude,
            height_km: height,
            frame,
            range_noise: Normal::new(0.0, range_noise).unwrap(),
            range_rate_noise: Normal::new(0.0, range_rate_noise).unwrap(),
        }
    }

    /// Initializes a point on the surface of a celestial object.
    /// This is meant for analysis, not for spacecraft navigation.
    pub fn from_point(
        name: String,
        latitude: f64,
        longitude: f64,
        height: f64,
        frame: Frame,
    ) -> Self {
        Self::from_noise_values(name, 0.0, latitude, longitude, height, 0.0, 0.0, frame)
    }

    pub fn dss65_madrid(
        elevation_mask: f64,
        range_noise: f64,
        range_rate_noise: f64,
        iau_earth: Frame,
    ) -> Self {
        Self::from_noise_values(
            "Madrid".to_string(),
            elevation_mask,
            40.427_222,
            4.250_556,
            0.834_939,
            range_noise,
            range_rate_noise,
            iau_earth,
        )
    }

    pub fn dss34_canberra(
        elevation_mask: f64,
        range_noise: f64,
        range_rate_noise: f64,
        iau_earth: Frame,
    ) -> Self {
        Self::from_noise_values(
            "Canberra".to_string(),
            elevation_mask,
            -35.398_333,
            148.981_944,
            0.691_750,
            range_noise,
            range_rate_noise,
            iau_earth,
        )
    }

    pub fn dss13_goldstone(
        elevation_mask: f64,
        range_noise: f64,
        range_rate_noise: f64,
        iau_earth: Frame,
    ) -> Self {
        Self::from_noise_values(
            "Goldstone".to_string(),
            elevation_mask,
            35.247_164,
            243.205,
            1.071_149_04,
            range_noise,
            range_rate_noise,
            iau_earth,
        )
    }
}

#[cfg_attr(feature = "python", pymethods)]
impl GroundStation {
    /// Computes the elevation of the provided object seen from this ground station.
    /// Also returns the ground station's orbit in the frame of the receiver
    pub fn elevation_of(&self, rx: &Orbit, cosm: &Cosm) -> (f64, Orbit, Orbit) {
        // Start by converting the receiver spacecraft into the ground station frame.
        let rx_gs_frame = cosm.frame_chg(rx, self.frame);

        let dt = rx.dt;
        // Then, compute the rotation matrix from the body fixed frame of the ground station to its topocentric frame SEZ.
        let tx_gs_frame = self.to_orbit(dt);
        // Note: we're only looking at the radis so we don't need to apply the transport theorem here.
        let dcm_topo2fixed = tx_gs_frame.dcm_from_traj_frame(Frame::SEZ).unwrap();

        // Now, rotate the spacecraft in the SEZ frame to compute its elevation as seen from the ground station.
        // We transpose the DCM so that it's the fixed to topocentric rotation.
        let rx_sez = rx_gs_frame.with_position_rotated_by(dcm_topo2fixed.transpose());
        let tx_sez = tx_gs_frame.with_position_rotated_by(dcm_topo2fixed.transpose());
        // Now, let's compute the range ρ.
        let rho_sez = rx_sez - tx_sez;

        // Finally, compute the elevation (math is the same as declination)
        let elevation = rho_sez.declination();

        // Return elevation in degrees and rx/tx in the inertial frame of the spacecraft
        (elevation, *rx, cosm.frame_chg(&tx_gs_frame, rx.frame))
    }

    /// Return this ground station as an orbit in its current frame
    pub fn to_orbit(&self, epoch: Epoch) -> Orbit {
        Orbit::from_geodesic(
            self.latitude_deg,
            self.longitude_deg,
            self.height_km,
            epoch,
            self.frame,
        )
    }
}

impl TrackingDataSim<Orbit, StdMeasurement> for GroundStation {
    /// Perform a measurement from the ground station to the receiver (rx).
    fn measure<R: Rng>(
        &mut self,
        rx: &Orbit,
        rng: &mut R,
        cosm: Arc<Cosm>,
    ) -> Option<StdMeasurement> {
        let (elevation, rx_rxf, tx_rxf) = self.elevation_of(rx, &cosm);

        Some(StdMeasurement::new(
            rx.dt,
            tx_rxf,
            rx_rxf,
            elevation >= self.elevation_mask_deg,
            &self.range_noise,
            &self.range_rate_noise,
        ))
    }
}

impl TrackingDataSim<Spacecraft, StdMeasurement> for GroundStation {
    /// Perform a measurement from the ground station to the receiver (rx).
    fn measure<R: Rng>(
        &mut self,
        sc_rx: &Spacecraft,
        rng: &mut R,
        cosm: Arc<Cosm>,
    ) -> Option<StdMeasurement> {
        let (elevation, rx_ssb, tx_ssb) = self.elevation_of(&sc_rx.orbit, &cosm);

        Some(StdMeasurement::new(
            rx_ssb.dt,
            tx_ssb,
            rx_ssb,
            elevation >= self.elevation_mask_deg,
            &self.range_noise,
            &self.range_rate_noise,
        ))
    }
}

impl fmt::Display for GroundStation {
    // Prints the Keplerian orbital elements with units
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[{}] {} (lat.: {:.2} deg    long.: {:.2} deg    alt.: {:.2} m)",
            self.frame,
            self.name,
            self.latitude_deg,
            self.longitude_deg,
            self.height_km * 1e3,
        )
    }
}
