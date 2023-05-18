import { Err, Ok, Result } from "../utils/result";
import { headers } from "./helper";
import { ApiError, parseError } from "./error";

export async function login(username: string, password: string): Promise<Result<null, ApiError>> {
    const res = await fetch("/api/v2/auth/login", {
        method: "post",
        body: JSON.stringify({ username: username, password: password }),
        headers,
    });
    if (res.status === 200) {
        return Ok(null);
    } else {
        return Err(await parseError(res));
    }
}

export async function logout(): Promise<Result<null, ApiError>> {
    const res = await fetch("/api/v2/auth/logout", {
        method: "get",
        headers,
    });
    if (res.status === 200) {
        return Ok(null);
    } else {
        return Err(await parseError(res));
    }
}
