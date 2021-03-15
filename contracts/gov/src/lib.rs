pub mod contract;

mod querier;
mod staking;
mod state;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod mock_querier;

#[cfg(all(target_arch = "wasm32", not(feature = "library")))]
cosmwasm_std::create_entry_points!(contract);
