const STORAGE_KEY = "oii-check-client-id";

function generateClientId() {
    if (window.crypto?.randomUUID) {
        return window.crypto.randomUUID();
    }

    const bytes = new Uint8Array(16);
    window.crypto.getRandomValues(bytes);
    return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

export function getClientId() {
    const existingId = localStorage.getItem(STORAGE_KEY);
    if (existingId) {
        return existingId;
    }

    const clientId = generateClientId();
    localStorage.setItem(STORAGE_KEY, clientId);
    return clientId;
}
