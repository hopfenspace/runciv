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

import { exists, mapValues } from '../runtime';
import type { ChatMember } from './ChatMember';
import {
    ChatMemberFromJSON,
    ChatMemberFromJSONTyped,
    ChatMemberToJSON,
} from './ChatMember';
import type { ChatMessage } from './ChatMessage';
import {
    ChatMessageFromJSON,
    ChatMessageFromJSONTyped,
    ChatMessageToJSON,
} from './ChatMessage';

/**
 * The response to a get chat
 * 
 * `messages` should be sorted by the datetime of `message.created_at`.
 * @export
 * @interface GetChatResponse
 */
export interface GetChatResponse {
    /**
     * 
     * @type {Array<ChatMember>}
     * @memberof GetChatResponse
     */
    members: Array<ChatMember>;
    /**
     * 
     * @type {Array<ChatMessage>}
     * @memberof GetChatResponse
     */
    messages: Array<ChatMessage>;
}

/**
 * Check if a given object implements the GetChatResponse interface.
 */
export function instanceOfGetChatResponse(value: object): boolean {
    let isInstance = true;
    isInstance = isInstance && "members" in value;
    isInstance = isInstance && "messages" in value;

    return isInstance;
}

export function GetChatResponseFromJSON(json: any): GetChatResponse {
    return GetChatResponseFromJSONTyped(json, false);
}

export function GetChatResponseFromJSONTyped(json: any, ignoreDiscriminator: boolean): GetChatResponse {
    if ((json === undefined) || (json === null)) {
        return json;
    }
    return {
        
        'members': ((json['members'] as Array<any>).map(ChatMemberFromJSON)),
        'messages': ((json['messages'] as Array<any>).map(ChatMessageFromJSON)),
    };
}

export function GetChatResponseToJSON(value?: GetChatResponse | null): any {
    if (value === undefined) {
        return undefined;
    }
    if (value === null) {
        return null;
    }
    return {
        
        'members': ((value.members as Array<any>).map(ChatMemberToJSON)),
        'messages': ((value.messages as Array<any>).map(ChatMessageToJSON)),
    };
}

