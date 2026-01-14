all: js assets

js: standalone_js/index.html
	@echo "Converting the HTML file to a JavaScript file..."
	python3 standalone_js/html_to_js.py standalone_js/index.html > dist/autoload.js

assets: js
	@echo "Overwriting dist/autoload.js with the new content..."
	cp dist/autoload.js backend/assets/autoload.js

clean:
	@echo "Cleaning up the dist directory..."
	rm -f dist/autoload.js

.PHONY: js assets clean
