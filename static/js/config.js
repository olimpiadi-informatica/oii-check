export const TESTS = [
    ["https://jsonplaceholder.typicode.com/posts/1", "sunt aut facere repellat", false],
    ["https://dummyjson.com/recipes", "mozzarella", false],
    ["https://api.github.com/", "current_user", false],
    ["https://touristfacts.dikson.xyz/?fact=29", "lightbulb", false],
    ["https://training.olinfo.it/ping", "pong", false],
];

export function getContentSrc(pathname) {
    return pathname.includes("esordienti") ? "/cms/round2-debutant" : "/cms/round2";
}
