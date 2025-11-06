import { HandCashConnect } from '@handcash/handcash-connect';

const appId = process.env.REACT_APP_HANDCASH_APP_ID || '';

let handCashConnect;
try {
  handCashConnect = new HandCashConnect({ appId });
} catch (error) {
  console.warn('HandCash SDK init failed, using fallback:', error);
}

export class HandCashService {
  getRedirectionUrl() {
    if (!handCashConnect) return null;
    return handCashConnect.getRedirectionUrl();
  }

  async getAccountFromAuthToken(authToken) {
    if (!handCashConnect) throw new Error('HandCash not initialized');
    return handCashConnect.getAccountFromAuthToken(authToken);
  }
}

export default new HandCashService();
