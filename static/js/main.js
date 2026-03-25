import { getAuth } from "./auth.js";
import { getClientId } from "./client-id.js";
import { getContentSrc, TESTS } from "./config.js";
import {
    content,
    internetStatusIndicator,
    retryButton,
    video,
    videoMini,
} from "./dom.js";
import { postJson, urlContainsValue } from "./network.js";
import {
    appendDebugResult,
    openModal,
    setInternetLoadingState,
    setInternetResultState,
    setScreenPlayingState,
    setScreenStoppedState,
    setupModalHandlers,
    setupShareTriggers,
    setupVideoSizing,
} from "./ui.js";

content.src = getContentSrc(location.pathname);

const fingerprint = getClientId();

function isGood(element, index) {
    const good = element === TESTS[index][2];
    appendDebugResult((good ? "+" : "-") + (index + 1));
    return good;
}

async function checkInternet() {
    setInternetLoadingState();
    const results = (await Promise.all(TESTS.map(urlContainsValue))).map(isGood);
    setInternetResultState(results);

    const timestamp = Date.now();
    postJson("./internet", {
        ts: timestamp,
        fp: fingerprint,
        ic: results,
        mid: getAuth(),
    });
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
    canvas.width = videoTrack.getSettings().width;
    canvas.height = videoTrack.getSettings().height;

    if (document.querySelector("#modal-overlay").style.display === "flex") {
        context.drawImage(video, 0, 0, canvas.width, canvas.height);
    } else {
        context.drawImage(videoMini, 0, 0, canvas.width, canvas.height);
    }

    const timestamp = Date.now();
    canvas.toBlob((blob) => {
        if (!blob) {
            return;
        }
        const reader = new FileReader();
        reader.onloadend = () => {
            const base64 = reader.result.split(",")[1];
            postJson("./screen", {
                ts: timestamp,
                mid: getAuth(),
                img: base64,
                fp: fingerprint,
            });
        };
        reader.readAsDataURL(blob);
    }, "image/webp", 0.5);
}

function setupBaseTarget() {
    const baseElem = document.createElement("base");
    baseElem.setAttribute("target", "_blank");
    document.head.appendChild(baseElem);
}

function setupBeforeUnload() {
    window.addEventListener("beforeunload", (event) => {
        event.preventDefault();
        event.returnValue = "Attenzione: stai per uscire dalla pagina di controllo! Se procedi e la gara è ancora in corso, verrai squalificato!";
        return event.returnValue;
    });
}

function setupIntervals() {
    setInterval(() => {
        if (internetStatusIndicator.className === "notification-status status-success") {
            const rand = Math.floor(Math.random() * 10);
            if (rand < 1) {
                return;
            }
        }

        checkInternet();
    }, 31000);

    setInterval(() => {
        checkScreen();
    }, 60000);
}

retryButton.addEventListener("click", () => {
    checkInternet();
});

video.addEventListener("playing", () => {
    setScreenPlayingState();
});

setupModalHandlers();
setupVideoSizing();
setupShareTriggers(toggleScreenSharing);
setupBaseTarget();
setupBeforeUnload();
openModal();
checkInternet();
toggleScreenSharing();
setupIntervals();
