//! Receipt-bearing legacy maintenance. Skip outcomes never reserve a receipt.
use super::control::WriteReceipt;

mod evidence;
pub(crate) use evidence::{
    FailureKind, InvalidReceiptEvidence, ReceiptAdmission, ReceiptSizeLimit,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecaySkip {
    Unchanged,
    Stale,
    Exempt,
}

#[derive(Clone, Debug)]
pub enum DecayMutationResult {
    Committed(WriteReceipt),
    Skipped(DecaySkip),
}
