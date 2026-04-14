export const CLIENT_ID_STORAGE_KEY = "oii-check-client-id";
export const INTERNET_CHECK_INTERVAL_MS = 10_000;
export const INTERNET_CHECK_MIN_DURATION_MS = 1000;
export const INTERNET_CHECK_TIMEOUT_MS = 15000;
export const SCREEN_CAPTURE_MAX_HEIGHT = 1024;
export const SCREEN_CAPTURE_MAX_WIDTH = 1280;
export const SCREEN_CHECK_INTERVAL_MS = 10_000;

export const TESTS = [
    ["https://jsonplaceholder.typicode.com/posts/1", "sunt aut facere repellat"],
    ["https://dummyjson.com/recipes", "mozzarella"],
    ["https://touristfacts.dikson.xyz/?fact=29", "lightbulb"],
    ["https://training.olinfo.it/ping", "pong"],
];

export function getContentSrc() {
    return "/content/";
}
