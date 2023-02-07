///! Ethereum bridge smart contracts.

pub mod bridge {
    ethers::contract::abigen!(
        Bridge,
        "res/Bridge.abi";
    );
}

pub mod governance {
    ethers::contract::abigen!(
        Governance,
        "res/Governance.abi";
    );
}
