import sys
import os
import bs4
import re

def html_2_js(html_content, typ="always"):
    bs = bs4.BeautifulSoup(html_content, 'html.parser')

    if typ == "always":
        final_js = """
const _style = document.createElement('style');
_style.type = 'text/css';
_style.appendChild(document.createTextNode(`{CSS}`));
document.getElementsByTagName('head')[0].appendChild(_style);

const _div = document.createElement('div');
_div.innerHTML = `{BODY}`;
document.getElementsByTagName('body')[0].appendChild(_div);

{JS}
        """
    elif typ == "onload":
        final_js = """
document.getElementsByTagName('body')[0].onload = function() {{
    const head  = document.getElementsByTagName('head')[0];
    const css_content = `{CSS}`;
    const style = document.createElement('style');
    style.type = 'text/css';
    style.appendChild(document.createTextNode(css_content));
    head.appendChild(style);

    const body_content = `{BODY}`;
    const body = document.getElementsByTagName('body')[0];
    const div = document.createElement('div');
    div.innerHTML = body_content;
    body.appendChild(div);

    {JS}
}};
        """
    
    css = bs.find_all('style')
    # Replace all style tags with a single css string
    if css:
        css = "\n".join([str(tag) for tag in css])
        css = css.replace("<style>", "").replace("</style>", "")

    js = bs.find_all('script')
    # Remove all script tags from the body
    for tag in js:
        tag.extract()

    # Replace all script tags with a single js string
    if js:
        js = "\n".join([str(tag) for tag in js])
        js = js.replace("<script>", "").replace("</script>", "")

    body = bs.find('body')

    # Rename all "name" attributes to "_prefix_name"
    prefix = os.urandom(8).hex()
    for tag in bs.find_all(attrs={"name": True}):
        tag['name'] = f"_{prefix}_{tag['name']}"

    # Change all "id" attributes to "_prefix_id"
    for tag in bs.find_all(attrs={"id": True}):
        tag['id'] = f"_{prefix}_{tag['id']}"
    
    # Change all "class" attributes to "_prefix_class"
    for tag in bs.find_all(attrs={"class": True}):
        tag['class'] = [f"_{prefix}_{cls}" for cls in tag['class']]

    # Now fix the css and js
    if css:
        assert "name=\"" not in css, "name=\" is not supported"      
        pattern = re.compile(r'(\.[a-zA-Z0-9\-.#:]+.*{)')
        # max 5 replacements
        css = pattern.sub(lambda m: f"._{prefix}_{m.group(1)[1:]}", css)
        css = pattern.sub(lambda m: f"._{prefix}_{m.group(1)[1:]}", css)
        css = pattern.sub(lambda m: f"._{prefix}_{m.group(1)[1:]}", css)
        css = pattern.sub(lambda m: f"._{prefix}_{m.group(1)[1:]}", css)
        css = pattern.sub(lambda m: f"._{prefix}_{m.group(1)[1:]}", css)
        pattern = re.compile(r'(\.[a-zA-Z0-9\-.#:]+.*})')
        # max 5 replacements
        css = pattern.sub(lambda m: f"._{prefix}_{m.group(1)[1:]}", css)
        css = pattern.sub(lambda m: f"._{prefix}_{m.group(1)[1:]}", css)
        css = pattern.sub(lambda m: f"._{prefix}_{m.group(1)[1:]}", css)
        css = pattern.sub(lambda m: f"._{prefix}_{m.group(1)[1:]}", css)
        css = pattern.sub(lambda m: f"._{prefix}_{m.group(1)[1:]}", css)
    if js:
        assert "getElementById" not in js, "getElementById is not supported"
        assert "getElementsByClassName" not in js, "getElementsByClassName is not supported"
        assert "getElementsByName" not in js, "getElementsByName is not supported"
        assert "getElementsByTagName" not in js, "getElementsByTagName is not supported"

        js_lines = js.splitlines()
        js_lines = [line.strip() for line in js_lines if line.strip()]
        js_lines = [line for line in js_lines if not line.strip().startswith("//")]
        # remove more comments
        js_lines = [line.split(" //")[0] if " //" in line else line for line in js_lines]
        new_js_lines = []
        for line in js_lines:
            if "className" in line:
                line = re.sub(r'\'([a-zA-Z0-9\(\-.#\s]+[\'"\s])', lambda m: f"'_{prefix}_{m.group(1)}", line)
                line = re.sub(r'"([a-zA-Z0-9\(\-.#\s]+[\'"\s])', lambda m: f'"_{prefix}_{m.group(1)}', line)
                line = re.sub(r' ([a-zA-Z0-9\(\-.#\s]+[\'"\s])', lambda m: f" _{prefix}_{m.group(1)}", line)
            elif "document.querySelector" in line:
                pattern = re.compile(r'([\'"][a-zA-Z0-9\(\-.#\s]*[#.])([a-zA-Z0-9\-.#:\s]+[\'" ])')
                line = pattern.sub(lambda m: f"{m.group(1)}_{prefix}_{m.group(2)}", line)
                line = pattern.sub(lambda m: f"{m.group(1)}_{prefix}_{m.group(2)}", line)
                line = pattern.sub(lambda m: f"{m.group(1)}_{prefix}_{m.group(2)}", line)
                line = pattern.sub(lambda m: f"{m.group(1)}_{prefix}_{m.group(2)}", line)
                line = pattern.sub(lambda m: f"{m.group(1)}_{prefix}_{m.group(2)}", line)
            new_js_lines.append(line)
        js_lines = new_js_lines
        js = "\n".join(js_lines)

    final_js = final_js.format(
        CSS=css.replace('"', '\\"').replace("'", "\\'"),
        BODY=str(body).replace('"', '\\"').replace("'", "\\'"),
        JS=js
    )

    # Remove all the comments
    final_js = re.sub(r'<!--.*?-->', '', final_js, flags=re.DOTALL)
    # # Remove all the new lines
    # final_js = re.sub(r'\n+', '', final_js)
    # Remove all the extra spaces
    final_js = re.sub(r'\s+', ' ', final_js)
    # Remove all the tabs
    final_js = re.sub(r'\t+', '', final_js)

    
    return final_js

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python html_to_js.py <path_to_html_file> [<path_to_js_file>]")
        sys.exit(1)

    html_file_path = sys.argv[1]

    if not os.path.isfile(html_file_path):
        print(f"Error: The file {html_file_path} does not exist.")
        sys.exit(1)

    with open(html_file_path, 'r') as file:
        html_content = file.read()

    js_content = html_2_js(html_content)
    if len(sys.argv) > 2:
        js_file_path = sys.argv[2]
        with open(js_file_path, 'w') as file:
            file.write(js_content)
    else:
        print(js_content)
