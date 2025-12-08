from pathlib import Path
import base64
import json
import re
import time

from flask import Flask, render_template, request

app = Flask(__name__, static_folder='assets', static_url_path='/assets')

DATA_FOLDER = Path("data")
if not DATA_FOLDER.exists():
    DATA_FOLDER.mkdir(parents=True)
elif not DATA_FOLDER.is_dir():
    raise Exception(f"{DATA_FOLDER} exists and is not a directory")

milli_time = lambda: int(round(time.time() * 1000))
json_ok = json.dumps({"status": "ok"})
json_error = json.dumps({"status": "error"})

TOKEN_RE = re.compile(r"[tb]-[a-z]{3}-[0-9]{3}")
def check_token_path(token):
    if not isinstance(token, str): return False
    if TOKEN_RE.fullmatch(token) is None: return False

    directory = DATA_FOLDER / token
    directory.mkdir(exist_ok=True)
    return True

@app.route('/')
def index():
    return render_template('index.html')

@app.route('/internet', methods=['POST'])
def internet():
    data = request.get_json()
    if any(key not in data for key in ['fp', 'ic', 'mid', 'ts']):
        return json_error
    token = data['mid']
    if not check_token_path(token):
        return json_error

    # Add server timestamp
    data['server_ts'] = milli_time()

    filename = DATA_FOLDER / token / "internet.json"
    with filename.open("a") as f:
        f.write(json.dumps(data) + "\n")
    return json_ok

@app.route('/screen', methods=['POST'])
def screen():
    data = request.get_json()
    if any(key not in data for key in ['img', 'mid']):
        return json_error
    token = data['mid']
    if not check_token_path(token):
        return json_error

    img = data['img']
    img = base64.b64decode(img)

    filename = DATA_FOLDER / token / (str(milli_time()) + ".webp")
    with filename.open("wb") as f:
        f.write(img)
    return json_ok

if __name__ == '__main__':
    app.run(debug=True)
