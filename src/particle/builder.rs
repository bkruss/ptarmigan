use rand::prelude::*;
use rand_distr::StandardNormal;
use crate::geometry::{ThreeVector, FourVector, StokesVector};
use super::{Species, Particle};
use super::dstr::RadialDistribution;

#[derive(Copy,Clone)]
pub struct BeamBuilder {
    species: Species,
    num: usize,
    pub weight: f64,
    normal_espec: Option<bool>,
    pub gamma: f64,
    pub sigma: f64,
    pub gamma_min: f64,
    gamma_max: f64,
    radial_dstr: RadialDistribution,
    pub sigma_z: f64,
    energy_chirp: f64,
    angle: f64,
    collision_plane_angle: f64,
    pub rms_div: f64,
    initial_z: f64,
    offset: ThreeVector,
    pub pol: StokesVector,
}

impl BeamBuilder {
    pub fn new(species: Species, num: usize) -> Self {
        BeamBuilder {
            species,
            num,
            weight: 1.0,
            normal_espec: None,
            gamma: 0.0,
            sigma: 0.0,
            gamma_min: 0.0,
            gamma_max: 0.0,
            radial_dstr: RadialDistribution::Uniform {r_max: 0.0},
            sigma_z: 0.0,
            energy_chirp: 0.0,
            angle: 0.0,
            collision_plane_angle: 0.0,
            rms_div: 0.0,
            initial_z: 0.0,
            offset: ThreeVector::new(0.0, 0.0, 0.0),
            pol: StokesVector::unpolarized(),
        }
    }

    pub fn with_initial_z(&self, initial_z: f64) -> Self {
        BeamBuilder {
            initial_z,
            ..*self
        }
    }

    pub fn with_weight(&self, weight: f64) -> Self {
        BeamBuilder {
            weight,
            ..*self
        }
    }

    pub fn with_normal_energy_spectrum(&self, gamma: f64, sigma: f64) -> Self {
        BeamBuilder {
            normal_espec: Some(true),
            gamma,
            sigma,
            ..*self
        }
    }

    pub fn with_bremsstrahlung_spectrum(&self, gamma_min: f64, gamma_max: f64) -> Self {
        BeamBuilder {
            normal_espec: Some(false),
            gamma_min,
            gamma_max,
            ..*self
        }
    }

    pub fn with_divergence(&self, rms_div: f64) -> Self {
        BeamBuilder {
            rms_div,
            ..*self
        }
    }

    pub fn with_collision_angle(&self, angle: f64) -> Self {
        BeamBuilder {
            angle,
            ..*self
        }
    }

    pub fn with_collision_plane_at(&self, angle: f64) -> Self {
        BeamBuilder {
            collision_plane_angle: angle,
            ..*self
        }
    }

    pub fn with_normally_distributed_xy(&self, sigma_x: f64, sigma_y: f64) -> Self {
        BeamBuilder {
            radial_dstr: RadialDistribution::Normal { sigma_x, sigma_y },
            ..*self
        }
    }

    pub fn with_trunc_normally_distributed_xy(&self, sigma_x: f64, sigma_y: f64, x_max: f64, y_max: f64) -> Self {
        BeamBuilder {
            radial_dstr: RadialDistribution::TruncNormal { sigma_x, sigma_y, x_max, y_max },
            ..*self
        }
    }

    pub fn with_uniformly_distributed_xy(&self, r_max: f64) -> Self {
        BeamBuilder {
            radial_dstr: RadialDistribution::Uniform { r_max },
            ..*self
        }
    }

    pub fn with_length(&self, sigma_z: f64) -> Self {
        BeamBuilder {
            sigma_z,
            ..*self
        }
    }

    pub fn with_offset(&self, offset: ThreeVector) -> Self {
        BeamBuilder {
            offset,
            ..*self
        }
    }

    pub fn with_energy_chirp(&self, rho: f64) -> Self {
        BeamBuilder {
            energy_chirp: rho,
            ..*self
        }
    }

    pub fn with_polarization(&self, sv: StokesVector) -> Self {
        BeamBuilder {
            pol: sv,
            ..*self
        }
    }

    #[cfg(feature = "hdf5-output")]
    pub fn transverse_dstr_is_normal(&self) -> bool {
        matches!(self.radial_dstr, RadialDistribution::Normal {..} | RadialDistribution::TruncNormal {..})
    }

    #[cfg(feature = "hdf5-output")]
    pub fn has_brem_spec(&self) -> bool {
        self.normal_espec.map(|b| !b).unwrap_or(false)
    }

    #[cfg(feature = "hdf5-output")]
    pub fn radius(&self) -> (f64, f64) {
        match self.radial_dstr {
            RadialDistribution::Normal { sigma_x, sigma_y: _ } => (sigma_x, std::f64::INFINITY),
            RadialDistribution::TruncNormal { sigma_x, sigma_y: _, x_max, y_max: _ } => (sigma_x, x_max),
            RadialDistribution::Uniform { r_max } => (r_max, r_max),
        }
    }

    pub fn build<R: Rng>(&self, rng: &mut R) -> Vec<Particle> {
        let normal_espec = self.normal_espec.expect("primary energy spectrum not specified");
        (0..self.num).into_iter()
            .map(|i| {
                // Sample gamma from relevant distribution
                let (gamma, dz) = if normal_espec {
                    loop {
                        // for correlated gamma and z
                        let rho = -self.energy_chirp;
                        let n0 = rng.sample::<f64,_>(StandardNormal);
                        let n1 = rng.sample::<f64,_>(StandardNormal);
                        let n2 = rho * n0 + (1.0 - rho * rho).sqrt() * n1;

                        let dz = self.sigma_z * n0;
                        let gamma = self.gamma + self.sigma * n2;
                        if gamma > 1.0 {
                            break (gamma, dz);
                        }
                    }
                } else { // brem spec
                    let x_min = self.gamma_min / self.gamma_max;
                    let y_max = 4.0 / (3.0 * x_min) - 4.0 / 3.0 + x_min;
                    let x = loop {
                        let x = x_min + (1.0 - x_min) * rng.gen::<f64>();
                        let u = rng.gen::<f64>();
                        let y = 4.0 / (3.0 * x) - 4.0 / 3.0 + x;
                        if u <= y / y_max {
                            break x;
                        }
                    };

                    let dz = self.sigma_z * rng.sample::<f64,_>(StandardNormal);
                    (x * self.gamma_max, dz)
                };

                let u = match self.species {
                    Species::Electron | Species::Positron => -(gamma * gamma - 1.0).sqrt(),
                    Species::Photon => -gamma,
                };

                let theta_x = self.angle + self.rms_div * rng.sample::<f64,_>(StandardNormal);
                let theta_y = self.rms_div * rng.sample::<f64,_>(StandardNormal);

                let u = ThreeVector::new(u * theta_x.sin() * theta_y.cos(), u * theta_y.sin(), u * theta_x.cos() * theta_y.cos());
                let u = u.rotate_around_z(self.collision_plane_angle);
                let u = match self.species {
                    Species::Electron | Species::Positron => FourVector::new(0.0, u[0], u[1], u[2]).unitize(),
                    Species::Photon => FourVector::lightlike(u[0], u[1], u[2]),
                };

                let (t, z) = if self.offset[2] >= 0.0 {
                    // beam is further away
                    (-self.initial_z, self.initial_z + self.offset[2] + dz)
                } else {
                    // beam is closer to focal plane, push backwards
                    (-self.initial_z - self.offset[2].abs(), self.initial_z + dz)
                };

                let (x, y) = self.radial_dstr.sample(rng);

                let (x, y) = (x + self.offset[0], y + self.offset[1]);
                let r = ThreeVector::new(x, y, z);
                let r = r.rotate_around_y(self.angle);
                let r = r.rotate_around_z(self.collision_plane_angle);
                let r = FourVector::new(t, r[0], r[1], r[2]);

                Particle::create(self.species, r)
                    .with_normalized_momentum(u)
                    .with_polarization(self.pol)
                    .with_weight(self.weight)
                    .with_id(i as u64)
                    .with_parent_id(i as u64)
            })
        .collect()
    }
}