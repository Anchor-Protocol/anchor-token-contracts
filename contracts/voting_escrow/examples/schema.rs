use cosmwasm_schema::{export_schema_with_title, remove_schemas, schema_for};
use std::env::current_dir;
use std::fs::create_dir_all;

use anchor_token::voting_escrow::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, LockInfoResponse, QueryMsg, UserSlopeResponse,
    UserUnlockPeriodResponse, VotingPowerResponse,
};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema_with_title(&schema_for!(InstantiateMsg), &out_dir, "InstantiateMsg");
    export_schema_with_title(&schema_for!(ExecuteMsg), &out_dir, "ExecuteMsg");
    export_schema_with_title(&schema_for!(QueryMsg), &out_dir, "QueryMsg");
    export_schema_with_title(
        &schema_for!(VotingPowerResponse),
        &out_dir,
        "VotingPowerResponse",
    );
    export_schema_with_title(
        &schema_for!(UserSlopeResponse),
        &out_dir,
        "UserSlopeResponse",
    );
    export_schema_with_title(
        &schema_for!(UserUnlockPeriodResponse),
        &out_dir,
        "UserUnlockPeriodResponse",
    );
    export_schema_with_title(&schema_for!(LockInfoResponse), &out_dir, "LockInfoResponse");
    export_schema_with_title(&schema_for!(ConfigResponse), &out_dir, "ConfigResponse");
}
