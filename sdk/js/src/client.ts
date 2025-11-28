import axios, { AxiosInstance } from 'axios';

export interface BsvBankConfig {
  baseUrl: string;
  apiKey?: string;
}

export class BsvBankClient {
  private client: AxiosInstance;
  
  constructor(config: BsvBankConfig) {
    this.client = axios.create({
      baseURL: config.baseUrl,
      headers: config.apiKey ? {
        'Authorization': `Bearer ${config.apiKey}`
      } : {}
    });
  }
  
  async createDeposit(req: CreateDepositRequest): Promise<Deposit> {
    const { data } = await this.client.post('/deposits', req);
    return data;
  }
  
  async getBalance(paymail: string): Promise<number> {
    const { data } = await this.client.get(`/balance/${paymail}`);
    return data.balance_satoshis;
  }
}

export interface CreateDepositRequest {
  user_paymail: string;
  amount_satoshis: number;
  txid: string;
  lock_duration_days: number;
}

export interface Deposit {
  id: string;
  paymail: string;
  amount_satoshis: number;
  status: string;
}
