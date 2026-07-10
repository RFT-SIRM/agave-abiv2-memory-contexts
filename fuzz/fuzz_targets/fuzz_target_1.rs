#![no_main]

mod core_state;

use arbitrary::Unstructured;
use core_state::CoreState;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    let mut state = CoreState::new();

    if let Ok(p) = u.int_in_range::<u64>(0..=5) {
        state.p = p;
    }
    if let Ok(g) = u.arbitrary::<i128>() {
        state.global_field = g % 100;
    }
    if let Ok(s) = u.arbitrary::<u128>() {
        state.total_supply = s % 1000;
    }

    state.total_minted = state.total_supply;
    state.total_burned = 0;

    let field_contrib = state.global_field.checked_mul(state.p as i128).unwrap_or(0);
    state.total_base_sum = (state.total_supply as i128).saturating_sub(field_contrib);

    if state.check_invariant().is_err() {
        return;
    }

    let steps = data.len().clamp(1, 8);
    for _ in 0..steps {
        let op = u.arbitrary::<u8>().unwrap_or(0) % 5;
        match op {
            0 => {
                let _ = state.register_participant();
            }
            1 => {
                let base_balance = (u.arbitrary::<i128>().unwrap_or(0) % 100) - 50;
                let _ = state.unregister_participant(base_balance);
            }
            2 => {
                let amount = (u.arbitrary::<u128>().unwrap_or(0) % 50) + 1;
                let edge_cost = (u.arbitrary::<i128>().unwrap_or(0) % 20) - 10;
                let _ = state.apply_transfer(0, 0, amount, edge_cost);
            }
            3 => {
                let amount = (u.arbitrary::<u128>().unwrap_or(0) % 50) + 1;
                let _ = state.redistribute_amount(amount);
            }
            4 => {
                let _ = state.apply_neg_entropy_tick();
            }
            _ => {}
        }

        if state.check_invariant().is_err() {
            panic!("invariant violated during fuzz: {:?}", state);
        }
    }
});
