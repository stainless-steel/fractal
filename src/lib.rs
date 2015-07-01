//! Multiscale modeling framework for the analysis and synthesis of
//! positive-valued, long-range-dependent processes.
//!
//! ## References
//!
//! 1. R. H. Riedi, M. Crouse, V. Ribeiro, and R. Baraniuk, “A multifractal
//!    wavelet model with application to network traffic,” Information Theory,
//!    IEEE Transactions on, vol. 45, no. 3, pp. 992–1018, Apr 1999.

// The implementation is based on:
// http://dsp.rice.edu/software/multifractal-wavelet-model

#[cfg(test)]
extern crate assert;

extern crate dwt;
extern crate probability;
extern crate statistics;

use probability::generator::Generator;
use probability::distribution::{self, Distribution};

pub type Error = &'static str;
pub type Result<T> = std::result::Result<T, Error>;

macro_rules! raise(
    ($message:expr) => (return Err($message));
);

/// A model.
pub trait Model {
    /// Draw a sample.
    fn sample<G>(&self, &mut G) -> Result<Vec<f64>> where G: Generator;
}

/// A beta model.
pub struct Beta {
    betas: Vec<f64>,
    mu: f64,
    sd: f64,
}

impl Beta {
    /// Fit a multifractal wavelet model with beta-distributed multipliers.
    ///
    /// `ncoarse` is the minimal number of scaling coefficients at the coarsest
    /// level needed for the estimation of the mean and standard deviation of
    /// the process.
    pub fn fit(data: &[f64], ncoarse: usize) -> Result<Beta> {
        if ncoarse == 0 {
            raise!("`ncoarse` should be positive");
        }

        let n = data.len();
        let nscale = {
            let nscale = (n as f64 / ncoarse as f64).log2().floor();
            if nscale < 1.0 {
                raise!("`ncoarse` is too high for the given amount of data");
            }
            nscale as usize
        };
        let ncoarse = (n as f64 / (1 << nscale) as f64).floor() as usize;

        let n = ncoarse * (1 << nscale);
        let mut data = (&data[0..n]).to_vec();

        dwt::forward(&mut data, &dwt::wavelet::Haar::new(), nscale);

        let var = (0..(nscale + 1)).map(|i| {
            let range = if i == 0 {
                0..ncoarse
            } else {
                (ncoarse * (1 << (i - 1)))..(ncoarse * (1 << i))
            };
            mean_square(&data[range])
        }).collect::<Vec<_>>();

        let (mu, sd) = {
            let data = &data[0..ncoarse];
            (statistics::mean(data), statistics::variance(data).sqrt())
        };

        let mut betas = Vec::with_capacity(nscale);
        for i in 0..nscale {
            let eta = var[i] / var[i + 1];
            let beta = if i == 0 {
                0.5 * eta - 0.5
            } else {
                0.5 * eta * (betas[i - 1] + 1.0) - 0.5
            };
            if beta <= 0.0 {
                raise!("the model is not appropriate for the data");
            }
            betas.push(beta);
        }

        Ok(Beta { betas: betas, mu: mu, sd: sd })
    }
}

impl Model for Beta {
    fn sample<G>(&self, generator: &mut G) -> Result<Vec<f64>> where G: Generator {
        let nscale = self.betas.len();

        let mut data = Vec::with_capacity(1 << nscale);
        unsafe { data.set_len(1 << nscale) };

        let gaussian = distribution::Gaussian::new(self.mu, self.sd);
        let scale = 0.5f64.powf(nscale as f64 / 2.0);
        let z = scale * gaussian.sample(generator);
        if z < 0.0 {
            raise!("the model is not appropriate for the data");
        }
        data[0] = z;

        for i in 0..nscale {
            let beta = distribution::Beta::new(self.betas[i], self.betas[i], -1.0, 1.0);
            for j in (0..(1 << i)).rev() {
                let x = data[j];
                let a = beta.sample(generator);
                data[2 * j + 0] = (1.0 + a) * x;
                data[2 * j + 1] = (1.0 - a) * x;
            }
        }

        Ok(data)
    }
}

#[inline]
fn mean_square(data: &[f64]) -> f64 {
    &data.iter().fold(0.0, |sum, &x| sum + x * x) / data.len() as f64
}

#[cfg(test)]
mod tests {
    use assert;
    use probability::generator;

    use {Beta, Model};

    #[test]
    fn fit() {
        let data = [
            4.018080337519417e-01, 7.596669169084191e-02, 2.399161535536580e-01,
            1.233189348351655e-01, 1.839077882824167e-01, 2.399525256649028e-01,
            4.172670690843695e-01, 4.965443032574213e-02, 9.027161099152811e-01,
            9.447871897216460e-01, 4.908640924680799e-01, 4.892526384000189e-01,
            3.377194098213772e-01, 9.000538464176620e-01, 3.692467811202150e-01,
            1.112027552937874e-01, 7.802520683211379e-01, 3.897388369612534e-01,
            2.416912859138327e-01, 4.039121455881147e-01, 9.645452516838859e-02,
            1.319732926063351e-01, 9.420505907754851e-01, 9.561345402298023e-01,
            5.752085950784656e-01, 5.977954294715582e-02, 2.347799133724063e-01,
            3.531585712220711e-01, 8.211940401979591e-01, 1.540343765155505e-02,
            4.302380165780784e-02, 1.689900294627044e-01, 6.491154749564521e-01,
            7.317223856586703e-01, 6.477459631363067e-01, 4.509237064309449e-01,
            5.470088922863450e-01, 2.963208056077732e-01, 7.446928070741562e-01,
            1.889550150325445e-01, 6.867754333653150e-01, 1.835111557372697e-01,
        ];

        let model = Beta::fit(&data, 5).unwrap();

        assert::close(&model.betas, &[
            1.635153583946054e+01, 2.793188701574629e+00, 3.739374677617142e+00,
        ], 1e-14);
        assert::close(&[model.mu], &[1.184252871226982e+00], 1e-14);
        assert::close(&[model.sd], &[4.466592147518644e-01], 1e-14);
    }

    #[test]
    fn sample() {
        let data = [
            4.983640519821430e-01, 9.597439585160811e-01, 3.403857266661332e-01,
            5.852677509797773e-01, 2.238119394911370e-01, 7.512670593056529e-01,
            2.550951154592691e-01, 5.059570516651424e-01, 6.990767226566860e-01,
            8.909032525357985e-01, 9.592914252054443e-01, 5.472155299638031e-01,
            1.386244428286791e-01, 1.492940055590575e-01, 2.575082541237365e-01,
            8.407172559836625e-01, 2.542821789715310e-01, 8.142848260688164e-01,
            2.435249687249893e-01, 9.292636231872278e-01, 3.499837659848087e-01,
            1.965952504312082e-01, 2.510838579760311e-01, 6.160446761466392e-01,
            4.732888489027293e-01, 3.516595070629968e-01, 8.308286278962909e-01,
            5.852640911527243e-01, 5.497236082911395e-01, 9.171936638298100e-01,
            2.858390188203735e-01, 7.572002291107213e-01, 7.537290942784953e-01,
            3.804458469753567e-01, 5.678216407252211e-01, 7.585428956306361e-02,
            5.395011866660715e-02, 5.307975530089727e-01, 7.791672301020112e-01,
            9.340106842291830e-01, 1.299062084737301e-01, 5.688236608721927e-01,
        ];

        let model = Beta::fit(&data, 5).unwrap();
        let data = model.sample(&mut generator::default()).unwrap();

        assert_eq!(data.len(), 8);
    }
}
