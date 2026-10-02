.PHONY: install uninstall doctor gui

install:
	@scripts/install.sh

uninstall:
	@scripts/install.sh --uninstall

doctor:
	@bin/cellar doctor

gui:
	npm install && npm run tauri dev
