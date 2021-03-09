/*
    Nyx, blazing fast astrodynamics
    Copyright (C) 2021 Christopher Rabotin <christopher.rabotin@gmail.com>

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

use super::hyperdual::linalg::norm;
use super::hyperdual::{Float, Hyperdual};
use super::{Frame, Orbit, OrbitDual};
use crate::dimensions::{Matrix2, Matrix3, Vector2, Vector3, U7};
use crate::md::StateParameter;
use crate::time::{Duration, Epoch, TimeUnit};
use crate::utils::between_0_360;
use crate::NyxError;

use std::convert::From;
use std::fmt;

/// Stores a B-Plane
#[derive(Copy, Clone, Debug)]
pub struct BPlane {
    /// The $B_T$ component, in kilometers
    pub b_t: Hyperdual<f64, U7>,
    /// The $B_R$ component, in kilometers
    pub b_r: Hyperdual<f64, U7>,
    /// The Linearized Time of Flight
    pub ltof_s: Hyperdual<f64, U7>,
    /// The B-Plane rotation matrix
    pub str_dcm: Matrix3<f64>,
    /// The frame in which this B Plane was computed
    pub frame: Frame,
    /// The time of computation
    pub epoch: Epoch,
}

impl BPlane {
    /// Returns a newly defined B-Plane if the orbit is hyperbolic.
    pub fn new(orbit_real: Orbit) -> Result<Self, NyxError> {
        if orbit_real.ecc() <= 1.0 {
            Err(NyxError::NotHyperbolic(
                "Orbit is not hyperbolic. Convert to target object first".to_string(),
            ))
        } else {
            // Convert to OrbitDual so we can target it
            let orbit = OrbitDual::from(orbit_real);

            let one = Hyperdual::from(1.0);
            let zero = Hyperdual::from(0.0);

            let e_hat = orbit.evec() / orbit.ecc().dual;
            let h_hat = orbit.hvec() / orbit.hmag().dual;
            let n_hat = h_hat.cross(&e_hat);

            // The reals implementation (which was initially validated) was:
            // let s = e_hat / orbit.ecc() + (1.0 - (1.0 / orbit.ecc()).powi(2)).sqrt() * n_hat;
            // let s_hat = s / s.norm();

            let s = Vector3::new(
                e_hat[0] / orbit.ecc().dual
                    + (one - (one / orbit.ecc().dual).powi(2)).sqrt() * n_hat[0],
                e_hat[1] / orbit.ecc().dual
                    + (one - (one / orbit.ecc().dual).powi(2)).sqrt() * n_hat[1],
                e_hat[2] / orbit.ecc().dual
                    + (one - (one / orbit.ecc().dual).powi(2)).sqrt() * n_hat[2],
            );
            let s_hat = s / norm(&s); // Just to make sure to renormalize everything

            // The reals implementation (which was initially validated) was:
            // let b_vec = orbit.semi_minor_axis()
            //     * ((1.0 - (1.0 / orbit.ecc()).powi(2)).sqrt() * e_hat
            //         - (1.0 / orbit.ecc() * n_hat));
            let b_vec = Vector3::new(
                orbit.semi_minor_axis().dual
                    * ((one - (one / orbit.ecc().dual).powi(2)).sqrt() * e_hat[0]
                        - (one / orbit.ecc().dual * n_hat[0])),
                orbit.semi_minor_axis().dual
                    * ((one - (one / orbit.ecc().dual).powi(2)).sqrt() * e_hat[1]
                        - (one / orbit.ecc().dual * n_hat[1])),
                orbit.semi_minor_axis().dual
                    * ((one - (one / orbit.ecc().dual).powi(2)).sqrt() * e_hat[2]
                        - (one / orbit.ecc().dual * n_hat[2])),
            );
            let t = s_hat.cross(&Vector3::new(zero, zero, one));
            let t_hat = t / norm(&t);
            let r_hat = s_hat.cross(&t_hat);

            // Build the rotation matrix from inertial to B Plane
            let str_rot = Matrix3::new(
                s_hat[0].real(),
                s_hat[1].real(),
                s_hat[2].real(),
                t_hat[0].real(),
                t_hat[1].real(),
                t_hat[2].real(),
                r_hat[0].real(),
                r_hat[1].real(),
                r_hat[2].real(),
            );

            Ok(BPlane {
                b_r: b_vec.dot(&r_hat),
                b_t: b_vec.dot(&t_hat),
                ltof_s: b_vec.dot(&s_hat) / orbit.vmag().dual,
                str_dcm: str_rot,
                frame: orbit.frame,
                epoch: orbit.dt,
            })
        }
    }

    pub fn b_dot_t(&self) -> f64 {
        self.b_t.real()
    }

    pub fn b_dot_r(&self) -> f64 {
        self.b_r.real()
    }

    pub fn ltof(&self) -> Duration {
        self.ltof_s.real() * TimeUnit::Second
    }

    /// Returns the B plane angle in degrees between 0 and 360
    pub fn angle(&self) -> f64 {
        between_0_360(self.b_dot_r().atan2(self.b_dot_t()).to_degrees())
    }

    /// Returns the B plane vector magnitude, in kilometers
    pub fn mag(&self) -> f64 {
        (self.b_dot_t().powi(2) + self.b_dot_r().powi(2)).sqrt()
    }

    /// Returns the DCM to convert to the B Plane from the inertial frame
    pub fn inertial_to_bplane(&self) -> Matrix3<f64> {
        self.str_dcm
    }

    /// Returns the **inverted** Jacobian of the B plane (BT, BR, LTOF) with respect to the velocity
    pub fn jacobian(&self) -> Matrix3<f64> {
        let mut jac = Matrix3::new(
            self.b_t[4],
            self.b_t[5],
            self.b_t[6],
            self.b_r[4],
            self.b_r[5],
            self.b_r[6],
            self.ltof_s[4],
            self.ltof_s[5],
            self.ltof_s[6],
        );

        jac.try_inverse_mut();
        jac
    }

    /// Returns the **inverted** Jacobian of the B plane (BT, BR) with respect to two of the velocity components
    pub fn jacobian2(&self, invariant: StateParameter) -> Result<Matrix2<f64>, NyxError> {
        let mut jac = match invariant {
            StateParameter::VX => Matrix2::new(self.b_t[5], self.b_t[6], self.b_r[5], self.b_r[6]),
            StateParameter::VY => Matrix2::new(self.b_t[4], self.b_t[6], self.b_r[4], self.b_r[6]),
            StateParameter::VZ => Matrix2::new(self.b_t[4], self.b_t[5], self.b_r[4], self.b_r[5]),
            _ => {
                return Err(NyxError::CustomError(
                    "B Plane jacobian invariant must be either VX, VY or VZ".to_string(),
                ))
            }
        };

        jac.try_inverse_mut();
        Ok(jac)
    }
}

impl fmt::Display for BPlane {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[{}] {} B-Plane: B∙R = {:.3} km\tB∙T = {:.3} km\tLTOF = {}",
            self.frame,
            self.epoch,
            self.b_dot_r(),
            self.b_dot_t(),
            self.ltof()
        )
    }
}

#[derive(Copy, Clone, Debug)]
pub struct BPlaneTarget {
    /// The $B_T$ component, in kilometers
    pub b_t_km: f64,
    /// The $B_R$ component, in kilometers
    pub b_r_km: f64,
    /// The Linearized Time of Flight, in seconds
    pub ltof_s: f64,

    /// The tolerance on the $B_T$ component, in kilometers
    pub tol_b_t_km: f64,
    /// The tolerance on the $B_R$ component, in kilometers
    pub tol_b_r_km: f64,
    /// The tolerance on the Linearized Time of Flight, in seconds
    pub tol_ltof_s: f64,
}

impl BPlaneTarget {
    /// Initializes a new B Plane target with only the targets and the default tolerances.
    /// Default tolerances are 1 millimeter in positions and 1 second in LTOF
    pub fn from_targets(b_t_km: f64, b_r_km: f64, ltof: Duration) -> Self {
        let tol_ltof: Duration = 6.0 * TimeUnit::Hour;
        Self {
            b_t_km,
            b_r_km,
            ltof_s: ltof.in_seconds(),
            tol_b_t_km: 1e-6,
            tol_b_r_km: 1e-6,
            tol_ltof_s: tol_ltof.in_seconds(),
        }
    }

    /// Initializes a new B Plane target with only the B Plane targets (not LTOF constraint) and the default tolerances.
    /// Default tolerances are 1 millimeter in positions. Here, the LTOF tolerance is set to 100 days.
    pub fn from_b_plane(b_t_km: f64, b_r_km: f64) -> Self {
        let ltof_tol: Duration = 100 * TimeUnit::Day;
        Self {
            b_t_km,
            b_r_km,
            ltof_s: 0.0,
            tol_b_t_km: 1e-6,
            tol_b_r_km: 1e-6,
            tol_ltof_s: ltof_tol.in_seconds(),
        }
    }

    pub fn ltof_target_set(&self) -> bool {
        self.ltof_s.abs() > 1e-10
    }
}

impl fmt::Display for BPlaneTarget {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "B-Plane target: B∙R = {:.3} km (+/- {:.1} m)\tB∙T = {:.3} km (+/- {:.1} m)\tLTOF = {} (+/- {})",
            self.b_r_km,
            self.tol_b_r_km * 1e-3,
            self.b_t_km,
            self.tol_b_t_km * 1e-3,
            self.ltof_s * TimeUnit::Second,
            self.tol_ltof_s * TimeUnit::Second
        )
    }
}

/// Returns the Delta V (in km/s) needed to achieve the B Plane specified by B dot R and B dot T.
/// If no LTOF target is set, this method will fix VX, VY and VZ successively and use the minimum of those as a seed for the LTOF variation finding.
/// If the 3x3 search is worse than any of the 2x2s, then a 2x2 will be returned.
/// This uses the hyperdual formulation of the Jacobian and will also vary the linearize time of flight (LTOF).
pub fn achieve_b_plane(orbit: Orbit, target: BPlaneTarget) -> Result<Vector3<f64>, NyxError> {
    let mut min_total_dv = Vector3::new(std::f64::INFINITY, std::f64::INFINITY, std::f64::INFINITY);
    let mut min_ltof_s = target.ltof_s;

    let mut target = target;
    // Search kind is 3 if we're searching with LTOF, 0 if VX invariant, 1 if VY invariance, 2 is VZ invariant.
    let search_kind = if target.ltof_target_set() { 3 } else { 0 };

    for cur_search in search_kind..=3 {
        let mut total_dv = Vector3::zeros();
        let mut attempt_no = 0;
        let max_iter = 10;
        let mut real_orbit = orbit;
        let mut ltof_s = std::f64::INFINITY;
        // If the error is not going down, we'll raise an error
        let mut prev_b_plane_err = std::f64::INFINITY;
        loop {
            if attempt_no > max_iter {
                if search_kind == 3 {
                    // We were searching with LTOF from the start, and that failed
                    return Err(NyxError::MaxIterReached(max_iter));
                } else {
                    // Let's just ignore this problem and continue
                    break;
                }
            }

            // Build current B Plane
            let b_plane = BPlane::new(real_orbit)?;

            // Check convergence
            let br_err = target.b_r_km - b_plane.b_dot_r();
            let bt_err = target.b_t_km - b_plane.b_dot_t();
            let ltof_err = if cur_search == 3 {
                target.ltof_s - b_plane.ltof_s.real()
            } else {
                0.0
            };

            if br_err.abs() < target.tol_b_r_km
                && bt_err.abs() < target.tol_b_t_km
                && ltof_err.abs() < target.tol_ltof_s
            {
                ltof_s = b_plane.ltof_s.real();
                break;
            }

            if cur_search == 3 {
                // Build the error vector
                let b_plane_err = Vector3::new(bt_err, br_err, ltof_err);
                if b_plane_err.norm() >= prev_b_plane_err {
                    if search_kind == 3 {
                        return Err(NyxError::CorrectionIneffective(
                            "LTOF enabled correction is failing. Try to not set an LTOF target"
                                .to_string(),
                        ));
                    } else {
                        break;
                    }
                }
                prev_b_plane_err = b_plane_err.norm();

                println!("b_plane_err = {}", b_plane_err.norm());
                println!("{}", b_plane.jacobian());

                // Compute the delta-v
                let dv = b_plane.jacobian() * b_plane_err;

                total_dv[0] += dv[0];
                total_dv[1] += dv[1];
                total_dv[2] += dv[2];

                println!("dv = [{:.4}\t{:.4}\t{:.4}]", dv[0], dv[1], dv[2]);

                // Rebuild a new orbit
                real_orbit.vx += dv[0];
                real_orbit.vy += dv[1];
                real_orbit.vz += dv[2];
            } else {
                // Sequential search
                let param = match cur_search {
                    0 => StateParameter::VX,
                    1 => StateParameter::VY,
                    2 => StateParameter::VZ,
                    _ => unreachable!(),
                };
                println!("{:?}", param);
                // Build the error vector
                let b_plane_err = Vector2::new(bt_err, br_err);
                println!("b_plane_err = {}", b_plane_err.norm());
                println!("{}", b_plane.jacobian2(param)?);

                // Compute the delta-v
                let dv = b_plane.jacobian2(param)? * b_plane_err;

                // And apply appropriately
                match param {
                    StateParameter::VX => {
                        total_dv[1] += dv[0];
                        total_dv[2] += dv[1];

                        // Rebuild a new orbit
                        real_orbit.vy += dv[0];
                        real_orbit.vz += dv[1];
                    }
                    StateParameter::VY => {
                        total_dv[0] += dv[0];
                        total_dv[2] += dv[1];

                        // Rebuild a new orbit
                        real_orbit.vx += dv[0];
                        real_orbit.vz += dv[1];
                    }
                    StateParameter::VZ => {
                        total_dv[0] += dv[0];
                        total_dv[1] += dv[1];

                        // Rebuild a new orbit
                        real_orbit.vx += dv[0];
                        real_orbit.vy += dv[1];
                    }
                    _ => unreachable!(),
                };
            }

            attempt_no += 1;
        }

        // Update the min dv
        if total_dv.norm() < min_total_dv.norm() {
            min_total_dv = total_dv;
            min_ltof_s = ltof_s;

            println!(
                "==> NEW = {:.3} km/s\t LTOF={}",
                min_total_dv.norm(),
                min_ltof_s * TimeUnit::Second
            );
        }

        // If this is the last 2x2 search, let's update the target with the best LTOF so far.
        if cur_search == 2 {
            target.ltof_s = min_ltof_s;
        }
    }
    Ok(min_total_dv)
}
