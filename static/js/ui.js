import {
    closeModalButton,
    debugElement,
    failedStatusIcon,
    floatingNotification,
    internetStatusIndicator,
    loadingStatusIcon,
    loadingStatusText,
    modalOverlay,
    retryButton,
    screenStatusIndicator,
    silhouette,
    silhouetteText,
    successStatusIcon,
    video,
    videoIcon,
    videoMini,
} from "./dom.js";

export function setInternetLoadingState() {
    successStatusIcon.style.display = "none";
    failedStatusIcon.style.display = "none";
    loadingStatusIcon.style.display = "flex";
    loadingStatusText.textContent = "Loading";
    debugElement.innerHTML = "Debug: ";
    debugElement.style.display = "none";
    retryButton.disabled = true;
}

export function setInternetResultState(results) {
    if (results.every((value) => value === true)) {
        successStatusIcon.style.display = "flex";
        internetStatusIndicator.className = "notification-status status-success";
    } else {
        failedStatusIcon.style.display = "flex";
        debugElement.style.display = "block";
        internetStatusIndicator.className = "notification-status status-failed";
    }
    retryButton.disabled = false;
    loadingStatusText.textContent = "Completato";
    loadingStatusIcon.style.display = "none";
}

export function appendDebugResult(text) {
    debugElement.innerHTML += text;
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

export function closeModal() {
    modalOverlay.style.display = "none";
    document.body.style.overflow = "auto";
    floatingNotification.style.display = "flex";
}

export function openModal() {
    modalOverlay.style.display = "flex";
    document.body.style.overflow = "hidden";
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
