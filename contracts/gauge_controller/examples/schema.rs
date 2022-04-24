use cosmwasm_schema::{export_schema, remove_schemas, schema_for};
use std::env::current_dir;
use std::fs::create_dir_all;

use anchor_token::gauge_controller::{
    AllGaugeAddrResponse, ConfigResponse, ExecuteMsg, GaugeAddrResponse, GaugeCountResponse,
    GaugeRelativeWeightAtResponse, GaugeRelativeWeightResponse, GaugeWeightAtResponse,
    GaugeWeightResponse, InstantiateMsg, QueryMsg, TotalWeightAtResponse, TotalWeightResponse,
};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(GaugeWeightResponse), &out_dir);
    export_schema(&schema_for!(GaugeWeightAtResponse), &out_dir);
    export_schema(&schema_for!(TotalWeightResponse), &out_dir);
    export_schema(&schema_for!(TotalWeightAtResponse), &out_dir);
    export_schema(&schema_for!(GaugeRelativeWeightResponse), &out_dir);
    export_schema(&schema_for!(GaugeRelativeWeightAtResponse), &out_dir);
    export_schema(&schema_for!(GaugeCountResponse), &out_dir);
    export_schema(&schema_for!(GaugeAddrResponse), &out_dir);
    export_schema(&schema_for!(AllGaugeAddrResponse), &out_dir);
    export_schema(&schema_for!(ConfigResponse), &out_dir);
    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
}