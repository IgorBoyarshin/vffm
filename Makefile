deploy:
	cargo build --release
	sudo rsync -avh target/release/vffm /usr/local/bin/
