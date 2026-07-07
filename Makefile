# concourse — build & native install (user-level; rpm/deb metadata in Cargo.toml for release day)
PREFIX ?= $(HOME)/.local
APPS   := $(PREFIX)/share/applications
ICONS  := $(PREFIX)/share/icons/hicolor

.PHONY: release icons install uninstall

release:
	cargo build --release

icons: packaging/concourse.svg
	rsvg-convert -w 256 -h 256 packaging/concourse.svg -o packaging/concourse-256.png
	rsvg-convert -w 128 -h 128 packaging/concourse.svg -o packaging/concourse-128.png
	rsvg-convert -w 64  -h 64  packaging/concourse.svg -o packaging/concourse-64.png
	rsvg-convert -w 48  -h 48  packaging/concourse.svg -o packaging/concourse-48.png
	rsvg-convert -w 32  -h 32  packaging/concourse.svg -o packaging/concourse-32.png

install: release icons
	install -Dm755 target/release/concourse $(PREFIX)/bin/concourse
	install -Dm644 packaging/concourse.desktop $(APPS)/concourse.desktop
	install -Dm644 packaging/concourse.svg $(ICONS)/scalable/apps/concourse.svg
	install -Dm644 packaging/concourse-256.png $(ICONS)/256x256/apps/concourse.png
	install -Dm644 packaging/concourse-128.png $(ICONS)/128x128/apps/concourse.png
	install -Dm644 packaging/concourse-64.png  $(ICONS)/64x64/apps/concourse.png
	install -Dm644 packaging/concourse-48.png  $(ICONS)/48x48/apps/concourse.png
	install -Dm644 packaging/concourse-32.png  $(ICONS)/32x32/apps/concourse.png
	-update-desktop-database $(APPS) 2>/dev/null || true
	-gtk-update-icon-cache -f $(ICONS) 2>/dev/null || true
	@echo "installed: $(PREFIX)/bin/concourse (+ menu entry + icons)"

uninstall:
	rm -f $(PREFIX)/bin/concourse $(APPS)/concourse.desktop \
	      $(ICONS)/scalable/apps/concourse.svg \
	      $(ICONS)/256x256/apps/concourse.png $(ICONS)/128x128/apps/concourse.png \
	      $(ICONS)/64x64/apps/concourse.png $(ICONS)/48x48/apps/concourse.png \
	      $(ICONS)/32x32/apps/concourse.png
	-update-desktop-database $(APPS) 2>/dev/null || true
