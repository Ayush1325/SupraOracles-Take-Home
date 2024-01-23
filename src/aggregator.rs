use dsa::{
    signature::{Signer, Verifier},
    Components, KeySize, VerifyingKey,
};
use rust_decimal::Decimal;
use tokio::sync::mpsc;

use crate::file::FileFormat;

pub use dsa::SigningKey;

pub struct AggMessage {
    avg: Decimal,
    sign: dsa::Signature,
}

impl AggMessage {
    const fn new(avg: Decimal, sign: dsa::Signature) -> Self {
        Self { avg, sign }
    }

    pub fn with_key(avg: Decimal, key: dsa::SigningKey) -> Self {
        let sign = key.sign(&avg.serialize());
        Self::new(avg, sign)
    }

    fn verify(&self, verify_key: &dsa::VerifyingKey) -> bool {
        verify_key.verify(&self.avg.serialize(), &self.sign).is_ok()
    }
}

pub fn key_gen() -> (SigningKey, VerifyingKey) {
    let mut csprng = rand::thread_rng();
    let components = Components::generate(&mut csprng, KeySize::DSA_2048_224);
    let signing_key = SigningKey::generate(&mut csprng, components);
    let verifying_key = signing_key.verifying_key().to_owned();

    (signing_key, verifying_key)
}

pub async fn aggregator(
    mut rx: mpsc::Receiver<AggMessage>,
    tx_file: mpsc::Sender<FileFormat>,
    verify_key: VerifyingKey,
) -> anyhow::Result<()> {
    let mut avg = Decimal::new(0, 0);
    let mut count = 0;

    while let Some(msg) = rx.recv().await {
        if !msg.verify(&verify_key) {
            panic!("Invalid signature");
        }
        avg += msg.avg;
        count += 1;
    }

    avg /= Decimal::from(count);
    println!("Cache Complete. The average USD price of BTC is: {avg}");

    tx_file
        .send(FileFormat::agg_avg(avg))
        .await
        .map_err(Into::into)
}
