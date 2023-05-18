/* tslint:disable */
/* eslint-disable */
/**
 * runciv
 * No description provided (generated by Openapi Generator https://github.com/openapitools/openapi-generator)
 *
 * The version of the OpenAPI document: 0.1.0
 * Contact: git@omikron.dev
 *
 * NOTE: This class is auto generated by OpenAPI Generator (https://openapi-generator.tech).
 * https://openapi-generator.tech
 * Do not edit the class manually.
 */


import * as runtime from '../runtime';
import type {
  ApiErrorResponse,
  GameStateResponse,
  GameUploadRequest,
  GameUploadResponse,
  GetGameOverviewResponse,
} from '../models';
import {
    ApiErrorResponseFromJSON,
    ApiErrorResponseToJSON,
    GameStateResponseFromJSON,
    GameStateResponseToJSON,
    GameUploadRequestFromJSON,
    GameUploadRequestToJSON,
    GameUploadResponseFromJSON,
    GameUploadResponseToJSON,
    GetGameOverviewResponseFromJSON,
    GetGameOverviewResponseToJSON,
} from '../models';

export interface GetGameRequest {
    uuid: string;
}

export interface PushGameUpdateRequest {
    uuid: string;
    gameUploadRequest: GameUploadRequest;
}

/**
 * 
 */
export class GamesApi extends runtime.BaseAPI {

    /**
     * Retrieves a single game which is currently open (actively played)  If the game has been completed or aborted, it will respond with a `GameNotFound` in `ApiErrorResponse`.
     * Retrieves a single game which is currently open (actively played)
     */
    async getGameRaw(requestParameters: GetGameRequest, initOverrides?: RequestInit | runtime.InitOverrideFunction): Promise<runtime.ApiResponse<GameStateResponse>> {
        if (requestParameters.uuid === null || requestParameters.uuid === undefined) {
            throw new runtime.RequiredError('uuid','Required parameter requestParameters.uuid was null or undefined when calling getGame.');
        }

        const queryParameters: any = {};

        const headerParameters: runtime.HTTPHeaders = {};

        const response = await this.request({
            path: `/api/v2/games/{uuid}`.replace(`{${"uuid"}}`, encodeURIComponent(String(requestParameters.uuid))),
            method: 'GET',
            headers: headerParameters,
            query: queryParameters,
        }, initOverrides);

        return new runtime.JSONApiResponse(response, (jsonValue) => GameStateResponseFromJSON(jsonValue));
    }

    /**
     * Retrieves a single game which is currently open (actively played)  If the game has been completed or aborted, it will respond with a `GameNotFound` in `ApiErrorResponse`.
     * Retrieves a single game which is currently open (actively played)
     */
    async getGame(requestParameters: GetGameRequest, initOverrides?: RequestInit | runtime.InitOverrideFunction): Promise<GameStateResponse> {
        const response = await this.getGameRaw(requestParameters, initOverrides);
        return await response.value();
    }

    /**
     * Retrieves an overview of all open games of a player  The response does not contain any full game state, but rather a shortened game state identified by its ID and state identifier. If the state (`game_data_id`) of a known game differs from the last known identifier, the server has a newer state of the game. The `last_activity` field is a convenience attribute and shouldn\'t be used for update checks.
     * Retrieves an overview of all open games of a player
     */
    async getOpenGamesRaw(initOverrides?: RequestInit | runtime.InitOverrideFunction): Promise<runtime.ApiResponse<GetGameOverviewResponse>> {
        const queryParameters: any = {};

        const headerParameters: runtime.HTTPHeaders = {};

        const response = await this.request({
            path: `/api/v2/games`,
            method: 'GET',
            headers: headerParameters,
            query: queryParameters,
        }, initOverrides);

        return new runtime.JSONApiResponse(response, (jsonValue) => GetGameOverviewResponseFromJSON(jsonValue));
    }

    /**
     * Retrieves an overview of all open games of a player  The response does not contain any full game state, but rather a shortened game state identified by its ID and state identifier. If the state (`game_data_id`) of a known game differs from the last known identifier, the server has a newer state of the game. The `last_activity` field is a convenience attribute and shouldn\'t be used for update checks.
     * Retrieves an overview of all open games of a player
     */
    async getOpenGames(initOverrides?: RequestInit | runtime.InitOverrideFunction): Promise<GetGameOverviewResponse> {
        const response = await this.getOpenGamesRaw(initOverrides);
        return await response.value();
    }

    /**
     * Upload a new game state for an existing game  If the game can\'t be updated (maybe it has been already completed or aborted), it will respond with a `GameNotFound` in `ApiErrorResponse`.
     * Upload a new game state for an existing game
     */
    async pushGameUpdateRaw(requestParameters: PushGameUpdateRequest, initOverrides?: RequestInit | runtime.InitOverrideFunction): Promise<runtime.ApiResponse<GameUploadResponse>> {
        if (requestParameters.uuid === null || requestParameters.uuid === undefined) {
            throw new runtime.RequiredError('uuid','Required parameter requestParameters.uuid was null or undefined when calling pushGameUpdate.');
        }

        if (requestParameters.gameUploadRequest === null || requestParameters.gameUploadRequest === undefined) {
            throw new runtime.RequiredError('gameUploadRequest','Required parameter requestParameters.gameUploadRequest was null or undefined when calling pushGameUpdate.');
        }

        const queryParameters: any = {};

        const headerParameters: runtime.HTTPHeaders = {};

        headerParameters['Content-Type'] = 'application/json';

        const response = await this.request({
            path: `/api/v2/games/{uuid}`.replace(`{${"uuid"}}`, encodeURIComponent(String(requestParameters.uuid))),
            method: 'PUT',
            headers: headerParameters,
            query: queryParameters,
            body: GameUploadRequestToJSON(requestParameters.gameUploadRequest),
        }, initOverrides);

        return new runtime.JSONApiResponse(response, (jsonValue) => GameUploadResponseFromJSON(jsonValue));
    }

    /**
     * Upload a new game state for an existing game  If the game can\'t be updated (maybe it has been already completed or aborted), it will respond with a `GameNotFound` in `ApiErrorResponse`.
     * Upload a new game state for an existing game
     */
    async pushGameUpdate(requestParameters: PushGameUpdateRequest, initOverrides?: RequestInit | runtime.InitOverrideFunction): Promise<GameUploadResponse> {
        const response = await this.pushGameUpdateRaw(requestParameters, initOverrides);
        return await response.value();
    }

}
