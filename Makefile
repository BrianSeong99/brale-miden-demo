.PHONY: check build test create-account deploy-issuer mint burn transfer read-balance read-supply full-demo full-demo-public full-demo-private multisig-demo

check:
	cargo check --workspace

build:
	cargo build --workspace

test:
	cargo test --workspace

create-account:
	cargo run -p integration --bin create_account

deploy-issuer:
	cargo run -p integration --bin deploy_issuer -- $(ARGS)

mint:
	cargo run -p integration --bin mint -- $(ARGS)

burn:
	cargo run -p integration --bin burn -- $(ARGS)

transfer:
	cargo run -p integration --bin transfer -- $(ARGS)

read-balance:
	cargo run -p integration --bin read_balance -- $(ARGS)

read-supply:
	cargo run -p integration --bin read_supply

full-demo-public:
	cargo run -p integration --bin full_demo

full-demo-private:
	cargo run -p integration --bin full_demo -- --private

full-demo: full-demo-public

multisig-demo:
	cargo run -p integration --bin multisig_demo
