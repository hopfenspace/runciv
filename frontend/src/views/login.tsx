import React from "react";
import { Api } from "../api/api";
import { toast } from "react-toastify";

import "../styling/login.css";
import Input from "../components/input";
import { handleApiError } from "../utils/helper";

type LoginProps = {
    onLogin(): void;
};
type LoginState = {
    username: string;
    password: string;
};

export default class Login extends React.Component<LoginProps, LoginState> {
    state: LoginState = {
        username: "",
        password: "",
    };

    performLogin() {
        Api.auth.login(this.state.username, this.state.password).then(
            handleApiError(() => {
                toast.success("Authenticated successfully");
                this.props.onLogin();
            })
        );
    }

    render() {
        return <div></div>;
    }
}
