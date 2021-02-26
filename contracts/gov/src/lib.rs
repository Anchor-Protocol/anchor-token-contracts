pub mod contract;
pub mod querier;
pub mod state;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod mock_querier;

#[cfg(all(target_arch = "wasm32", not(feature = "library")))]
cosmwasm_std::create_entry_points!(contract);
