export const content = document.getElementById("content");
export const retryButton = document.querySelector("#retry");
export const video = document.querySelector(".video-preview");
export const videoMini = document.querySelector(".video-mini");
export const modalOverlay = document.querySelector("#modal-overlay");

const closeModalButton = document.querySelector("#close-modal");
const debugElement = document.querySelector("#debug");
const failedStatusIcon = document.querySelector(".failed .status-icon");
const floatingNotification = document.querySelector("#floating-notification");
const internetStatusIndicator = document.querySelector("#internet-status-indicator");
const loadingStatusIcon = document.querySelector(".loading .status-icon");
const loadingStatusText = document.querySelector(".loading .status-text");
const screenStatusIndicator = document.querySelector("#screen-status-indicator");
const silhouette = document.querySelector(".silhouette");
const silhouetteText = document.querySelector(".silhouette-text");
const successStatusIcon = document.querySelector(".success .status-icon");
const videoIcon = document.querySelector(".video-icon");

export function setInternetLoadingState() {
    successStatusIcon.style.display = "none";
    failedStatusIcon.style.display = "none";
    loadingStatusIcon.style.display = "flex";
    loadingStatusText.textContent = "Loading";
    debugElement.textContent = "";
    retryButton.disabled = true;
}

export function setInternetResultState(results) {
    debugElement.textContent = "Debug: " + results.map((good, index) => `${good ? "o" : "x"}${index + 1}`).join("-");

    if (results.every((value) => value === true)) {
        successStatusIcon.style.display = "flex";
        internetStatusIndicator.className = "notification-status status-success";
    } else {
        failedStatusIcon.style.display = "flex";
        internetStatusIndicator.className = "notification-status status-failed";
    }
    retryButton.disabled = false;
    loadingStatusText.textContent = "Completato";
    loadingStatusIcon.style.display = "none";
}

export function setScreenStoppedState() {
    screenStatusIndicator.className = "notification-status status-failed";
    silhouette.style.display = "flex";
    silhouetteText.style.display = "block";
    videoMini.style.display = "none";
    videoIcon.style.display = "flex";
    video.srcObject = null;
    videoMini.srcObject = null;
}

export function setScreenPlayingState() {
    silhouette.style.display = "none";
    silhouetteText.style.display = "none";
    screenStatusIndicator.className = "notification-status status-success";
    videoMini.style.display = "flex";
    videoIcon.style.display = "none";
}

function closeModal() {
    modalOverlay.style.display = "none";
    floatingNotification.style.display = "flex";
}

function openModal() {
    modalOverlay.style.display = "flex";
    floatingNotification.style.display = "none";
}

export function setupModalHandlers() {
    closeModalButton.addEventListener("click", closeModal);
    modalOverlay.addEventListener("click", (event) => {
        if (event.target === modalOverlay) {
            closeModal();
        }
    });
    document.addEventListener("keydown", (event) => {
        if (event.key === "Escape") {
            closeModal();
        }
    });
    floatingNotification.addEventListener("click", openModal);
}

export function setupVideoSizing() {
    const currWidth = video.clientWidth;
    const aspectRatio = window.screen.width / window.screen.height;
    video.style.width = `${currWidth}px`;
    video.style.height = `${currWidth / aspectRatio}px`;
}

export function setupShareTriggers(toggleScreenSharing) {
    video.addEventListener("click", toggleScreenSharing);
    silhouette.addEventListener("click", toggleScreenSharing);
    silhouetteText.addEventListener("click", toggleScreenSharing);
}
