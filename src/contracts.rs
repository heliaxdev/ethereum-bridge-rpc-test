///! Ethereum bridge smart contracts.

pub mod bridge {
    ethers::contract::abigen!(
        Bridge,
        "res/Bridge.abi";
    );
}

pub mod test_erc20 {
    ethers::contract::abigen!(
        TestERC20,
        "res/TestERC20.abi";
    );
}

pub mod governance {
    ethers::contract::abigen!(
        Governance,
        "res/Governance.abi";
    );
}
