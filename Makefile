.PHONY: check build test create-account deploy-issuer mint burn transfer read-balance read-supply full-demo multisig-demo

check:
	cargo check --workspace

build:
	cargo build --workspace

test:
	cargo test --workspace

create-account:
	cargo run -p integration --bin create_account

deploy-issuer:
	cargo run -p integration --bin deploy_issuer

mint:
	cargo run -p integration --bin mint

burn:
	cargo run -p integration --bin burn

transfer:
	cargo run -p integration --bin transfer

read-balance:
	cargo run -p integration --bin read_balance

read-supply:
	cargo run -p integration --bin read_supply

full-demo:
	cargo run -p integration --bin full_demo

multisig-demo:
	cargo run -p integration --bin multisig_demo
