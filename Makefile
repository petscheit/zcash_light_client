setup:
	./scripts/setup.sh

activate:
	@echo "Please source the virtual environment activation script:"
	@echo "  source scripts/activate.sh"

build-cairo:
	./scripts/cairo_compile.sh cairo/src/main.cairo

get-program-hash:
	# @make build
	@echo "CairoProgramHash:"
	@cairo-hash-program --program cairo/build/main.json