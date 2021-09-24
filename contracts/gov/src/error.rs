use cosmwasm_std::{OverflowError, StdError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    OverflowError(#[from] OverflowError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Asset mismatch")]
    AssetMismatch {},

    #[error("Data should be given")]
    DataShouldBeGiven {},

    #[error("Insufficient funds sent")]
    InsufficientFunds {},

    #[error("Must deposit more than {0} token")]
    InsufficientProposalDeposit(u128),

    #[error("Reward deposited is too small")]
    InsufficientReward {},

    #[error("User does not have enough staked tokens")]
    InsufficientStaked {},

    #[error("Nothing staked")]
    NothingStaked {},

    #[error("User is trying to withdraw too many tokens")]
    InvalidWithdrawAmount {},

    #[error("Nothing to withdraw")]
    NothingToWithdraw {},

    #[error("Poll does not exist")]
    PollNotFound {},

    #[error("Snapshot has already occurred")]
    SnapshotAlreadyOccurred {},

    #[error("Timelock period has not expired")]
    TimelockNotExpired {},

    #[error("Poll is not in progress")]
    PollNotInProgress {},

    #[error("Poll is not in passed status")]
    PollNotPassed {},

    #[error("Cannot snapshot at this height")]
    SnapshotHeight {},

    #[error("User has already voted")]
    AlreadyVoted {},

    #[error("Expire height has not been reached")]
    PollNotExpired {},

    #[error("Voting period has not expired")]
    PollVotingPeriod {},

    #[error("Invalid Reply Id")]
    InvalidReplyId {},
}
