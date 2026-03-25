export function getAuth() {
    const cookie = document.cookie
        .split("; ")
        .find((row) => row.split("=")[0].endsWith("_login"))
        ?.split("=")[1];
    if (!cookie) return null;
    const username = JSON.parse(atob(cookie.split("|")[4].split(":")[1]))[0];
    return username;
}
