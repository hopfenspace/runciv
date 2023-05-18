import React from "react";
import ReactDOM from "react-dom/client";
import "./index.css";
import { ROUTER } from "./routes.tsx";
import { UserProvider } from "./context/user.tsx";
import { ToastContainer } from "react-toastify";
import "react-toastify/dist/ReactToastify.css";

type RouterProps = {};
type RouterState = {
    path: Array<string>;
};

class Router extends React.Component<RouterProps, RouterState> {
    state: RouterState = {
        path: [],
    };

    componentDidMount() {
        // Update state to match url
        const setPath = () => {
            const rawPath = window.location.hash;

            // Ensure well-formed path i.e. always have a #/
            if (!rawPath.startsWith("#/")) {
                window.location.hash = "#/";

                // this method will be immediately triggered again
                return;
            }

            // Split everything after #/
            const path = rawPath.substring(2).split("/");

            // #/ should result in [] not [""]
            if (path.length === 1 && path[0] === "") {
                path.shift();
            }

            this.setState({ path });
        };

        setPath();
        window.addEventListener("hashchange", setPath);
    }

    render() {
        return (
            <UserProvider>
                <div className="content-container">
                    {ROUTER.matchAndRender(this.state.path) || <div>Unknown route</div>}
                </div>
            </UserProvider>
        );
    }
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <>
        <Router />
        <ToastContainer autoClose={3500} theme="dark" />
    </>
);
