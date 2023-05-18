import {
    AccountRegistrationRequest,
    AccountsApi,
    ChatsApi,
    Configuration,
    CreateInviteRequest,
    CreateLobbyRequest,
    FriendsApi,
    GamesApi,
    InvitesApi,
    LobbiesApi,
    UpdateAccountRequest,
} from "./generated";
import { handleError } from "./error.ts";
import { login, logout } from "./auth.ts";

/** Database id i.e. and u32 */
export type ID = number;

/** Hyphen separated uuid */
export type UUID = string;

const configuration = new Configuration({
    basePath: window.location.origin,
});
const accounts = new AccountsApi(configuration);
const chats = new ChatsApi(configuration);
const friends = new FriendsApi();
const games = new GamesApi();
const invites = new InvitesApi();
const lobbies = new LobbiesApi();
export const Api = {
    auth: {
        login,
        logout,
    },
    accounts: {
        register: (account: AccountRegistrationRequest) =>
            accounts.registerAccount({ accountRegistrationRequest: account }),
        getMe: () => handleError(accounts.getMe()),
        updateMe: (update: UpdateAccountRequest) => handleError(accounts.updateMe({ updateAccountRequest: update })),
        deleteMe: () => handleError(accounts.deleteMe()),
        setPassword: (oldPassword: string, newPassword: string) =>
            handleError(accounts.setPassword({ setPasswordRequest: { newPassword, oldPassword } })),
        lookupByUsername: (username: string) =>
            handleError(accounts.lookupAccountByUsername({ lookupAccountUsernameRequest: { username } })),
        lookupByUuid: (uuid: UUID) => handleError(accounts.lookupAccountByUuid({ uuid })),
    },
    chats: {
        getAll: () => handleError(chats.getAllChats()),
        get: (uuid: UUID) => handleError(chats.getChat({ uuid })),
        send: (uuid: UUID, message: string) =>
            handleError(chats.sendMessage({ uuid, sendMessageRequest: { message } })),
    },
    friends: {
        getAll: () => handleError(friends.getFriends()),
        create: (uuid: UUID) => handleError(friends.createFriendRequest({ createFriendRequest: { uuid } })),
        accept: (uuid: UUID) => handleError(friends.acceptFriendRequest({ uuid })),
        delete: (uuid: UUID) => handleError(friends.deleteFriend({ uuid })),
    },
    games: {
        getAll: () => handleError(games.getOpenGames()),
        get: (uuid: UUID) => handleError(games.getGame({ uuid })),
        update: (uuid: UUID, gameData: string) =>
            handleError(games.pushGameUpdate({ uuid, gameUploadRequest: { gameData } })),
    },
    invites: {
        get: () => handleError(invites.getInvites()),
        create: (invite: CreateInviteRequest) => handleError(invites.createInvite({ createInviteRequest: invite })),
        delete: (uuid: UUID) => handleError(invites.deleteInvite({ uuid })),
        accept: (uuid: UUID) => handleError(invites.acceptInvite({ uuid })),
    },
    lobbies: {
        getAll: () => handleError(lobbies.getAllLobbies()),
        get: (uuid: UUID) => handleError(lobbies.getLobby({ uuid })),
        create: (lobby: CreateLobbyRequest) => handleError(lobbies.createLobby({ createLobbyRequest: lobby })),
        delete: (uuid: UUID) => handleError(lobbies.closeLobby({ uuid })),
        kick: (lobbyUuid: UUID, playerUuid: UUID) =>
            handleError(lobbies.kickPlayerFromLobby({ lobbyUuid, playerUuid })),
        join: (uuid: UUID, password: string | null | undefined) =>
            handleError(lobbies.joinLobby({ uuid, joinLobbyRequest: { password } })),
        leave: (uuid: UUID) => handleError(lobbies.leaveLobby({ uuid })),
        start: (uuid: UUID) => handleError(lobbies.startGame({ uuid })),
    },
};
