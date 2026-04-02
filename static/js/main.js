import {
    CLIENT_ID_STORAGE_KEY,
    getContentSrc,
    INTERNET_CHECK_INTERVAL_MS,
    INTERNET_CHECK_MIN_DURATION_MS,
    INTERNET_CHECK_TIMEOUT_MS,
    SCREEN_CAPTURE_MAX_HEIGHT,
    SCREEN_CAPTURE_MAX_WIDTH,
    SCREEN_CHECK_INTERVAL_MS,
    TESTS,
} from "./config.js";
import {
    content,
    modalOverlay,
    openModal,
    retryButton,
    setInternetLoadingState,
    setInternetResultState,
    setScreenPlayingState,
    setScreenStoppedState,
    setupModalHandlers,
    setupShareTriggers,
    setupVideoSizing,
    video,
    videoMini,
} from "./ui.js";

content.src = getContentSrc();

function isValidClientId(value) {
    return /^[0-9a-f]{32}$/.test(value);
}

function generateClientId() {
    const bytes = crypto.getRandomValues(new Uint8Array(16));
    return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function getClientId() {
    const existingId = localStorage.getItem(CLIENT_ID_STORAGE_KEY);
    if (existingId && isValidClientId(existingId)) {
        return existingId;
    }

    const clientId = generateClientId();
    localStorage.setItem(CLIENT_ID_STORAGE_KEY, clientId);
    return clientId;
}

const fingerprint = getClientId();
const pendingRequests = [];
let isFlushingRequests = false;

function unixTimestampSeconds() {
    return Date.now() / 1000;
}

async function urlContainsValue([url, value]) {
    try {
        const [resp] = await Promise.all([
            fetch(url, {
                signal: AbortSignal.timeout(INTERNET_CHECK_TIMEOUT_MS),
                cache: "no-store",
            }),
            new Promise((resolve) => setTimeout(resolve, INTERNET_CHECK_MIN_DURATION_MS)),
        ]);
        if (!resp.ok) return false;
        const text = await resp.text();
        return text.includes(value);
    } catch {
        return false;
    }
}

async function flushPendingRequests() {
    if (isFlushingRequests) {
        return;
    }

    isFlushingRequests = true;
    try {
        while (pendingRequests.length > 0) {
            const request = pendingRequests[0];
            let response;
            try {
                response = await fetch(request.url, request.options);
            } catch {
                return;
            }

            if (!response.ok) {
                return;
            }

            pendingRequests.shift();
        }
    } finally {
        isFlushingRequests = false;
    }
}

function postRequest(url, clientTs, fp, body, contentType) {
    pendingRequests.push({
        url,
        options: {
            method: "POST",
            headers: {
                "Content-Type": contentType,
                "X-OII-CLIENT-TS": String(clientTs),
                "X-OII-FP": fp,
            },
            cors: "cors",
            body,
        },
    });
    return flushPendingRequests();
}

function canvasToBlob(canvas, type, quality) {
    return new Promise(resolve => canvas.toBlob(resolve, type, quality));
}

async function encodeImage(canvas) {
    const list = await Promise.all([
        canvasToBlob(canvas, "image/png"),
        canvasToBlob(canvas, "image/jpeg", 0.95),
    ]);
    return list.filter(x => x !== null).reduce((a, b) => { return b.size < a.size ? b : a; });
}

async function checkInternet() {
    const timestamp = unixTimestampSeconds();

    setInternetLoadingState();
    const results = (await Promise.all(TESTS.map(urlContainsValue))).map((x) => !x);
    setInternetResultState(results);

    postRequest(
        "./internet",
        timestamp,
        fingerprint,
        JSON.stringify({ ic: results }),
        "application/json",
    );
}

function checkStream(stream) {
    const track = stream.getVideoTracks()[0];
    let isFullSurface = track.getSettings().displaySurface === "monitor";

    if (track.getSettings().displaySurface === undefined) {
        isFullSurface = window.screen.width <= track.getSettings().width &&
            window.screen.height <= track.getSettings().height;
    }

    if (!isFullSurface) {
        setTimeout(() => {
            alert("Assicurati di condividere l'intero schermo.");
        }, 100);
    }
}

function toggleScreenSharing() {
    if (video.srcObject && video.srcObject.active) {
        video.srcObject.getTracks().forEach((track) => track.stop());
        setScreenStoppedState();
        return;
    }

    navigator.mediaDevices.getDisplayMedia({
        video: {
            displaySurface: "monitor",
        },
        audio: false
    })
        .then((stream) => {
            checkStream(stream);
            video.srcObject = stream;
            videoMini.srcObject = stream;
            stream.getVideoTracks()[0].onended = () => {
                setScreenStoppedState();
            };
        })
        .catch((err) => {
            console.error("Error: " + err);
            setScreenStoppedState();
        });
}

async function checkScreen() {
    if (!video.srcObject || !video.srcObject.active) {
        return;
    }

    const videoTrack = video.srcObject.getVideoTracks()[0];
    const canvas = document.createElement("canvas");
    const context = canvas.getContext("2d");
    const sourceWidth = videoTrack.getSettings().width;
    const sourceHeight = videoTrack.getSettings().height;
    const scale = Math.min(
        1,
        SCREEN_CAPTURE_MAX_WIDTH / sourceWidth,
        SCREEN_CAPTURE_MAX_HEIGHT / sourceHeight,
    );
    canvas.width = Math.round(sourceWidth * scale);
    canvas.height = Math.round(sourceHeight * scale);

    if (modalOverlay.style.display === "flex") {
        context.drawImage(video, 0, 0, canvas.width, canvas.height);
    } else {
        context.drawImage(videoMini, 0, 0, canvas.width, canvas.height);
    }

    const timestamp = unixTimestampSeconds();
    const blob = await encodeImage(canvas);
    postRequest("./screen", timestamp, fingerprint, blob, blob.type);
}

window.addEventListener("beforeunload", (event) => {
    event.preventDefault();
    event.returnValue = "Attenzione: stai per uscire dalla pagina di controllo! Se procedi e la gara è ancora in corso, verrai squalificato!";
    return event.returnValue;
});

retryButton.addEventListener("click", () => {
    checkInternet();
});

video.addEventListener("playing", () => {
    setScreenPlayingState();
});

setInterval(checkInternet, INTERNET_CHECK_INTERVAL_MS);
setInterval(checkScreen, SCREEN_CHECK_INTERVAL_MS);

setupModalHandlers();
setupVideoSizing();
setupShareTriggers(toggleScreenSharing);
openModal();
checkInternet();
toggleScreenSharing();
