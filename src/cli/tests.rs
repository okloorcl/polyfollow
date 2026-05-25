use clap::Parser as _;

use super::Cli;

#[test]
fn leader_update_rejects_conflicting_sizing_modes() {
    let result = Cli::try_parse_from([
        "polyfollow",
        "leader",
        "update",
        "0x2222222222222222222222222222222222222222",
        "--copy-ratio",
        "0.10",
        "--fixed-order",
        "10",
    ]);

    assert!(result.is_err());
}
