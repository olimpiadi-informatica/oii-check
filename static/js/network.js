export async function urlContainsValue([url, value]) {
    try {
        const [resp] = await Promise.all([
            fetch(url, {
                signal: AbortSignal.timeout(15000),
                cache: "no-store",
            }),
            new Promise((resolve) => setTimeout(resolve, 1000)),
        ]);
        if (!resp.ok) return false;
        const text = await resp.text();
        return text.includes(value);
    } catch {
        return false;
    }
}

export function postJson(url, body) {
    return fetch(url, {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            "Accept": "application/json"
        },
        cors: "cors",
        body: JSON.stringify(body)
    });
}
