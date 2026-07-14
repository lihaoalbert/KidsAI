// W10 — Skill manifest verifier
//
// 单一入口: verify_skill_manifest(canonical, signature, pubkey_id).
// 复用 license_signer 单例. Skills 永远 RSA-PSS-SHA256, 与 secrets manifest 一致.

use crate::license_signer::{LicenseSigner, SignerError};

pub use crate::skills::{SkillManifestFull as SkillManifest};

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("signer not initialized (LicenseSigner missing)")]
    NoSigner,
    #[error("verify: {0}")]
    Signer(#[from] SignerError),
}

pub fn verify_skill_manifest(
    canonical: &str,
    signature_b64: &str,
    pubkey_id: &str,
) -> Result<(), VerifyError> {
    let signer = LicenseSigner::get().ok_or(VerifyError::NoSigner)?;
    signer.verify(canonical, signature_b64, pubkey_id)?;
    Ok(())
}